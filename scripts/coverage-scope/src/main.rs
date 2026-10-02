//! Partition LLVM's own executable-line counts using parsed Rust test scopes.
//! Never exclude production just because a function or filename contains `test`.
use proc_macro2::Span;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use syn::{Meta, spanned::Spanned, visit::Visit};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: trontop-coverage-scope <repository> <coverage-run>".into());
    }
    let repo = Path::new(&args[0]).canonicalize()?;
    let run = Path::new(&args[1]);
    let files = source_scopes(&repo.join("src/main.rs"))?;
    let lcov = parse_lcov(&std::fs::read_to_string(run.join("coverage.lcov"))?)?;
    let llvm: Value = serde_json::from_str(&std::fs::read_to_string(run.join("coverage.json"))?)?;
    let report = partition(&repo, &files, &lcov, &llvm)?;
    std::fs::write(
        run.join("production-lines.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    println!(
        "COVERAGE: production executable lines {}/{} ({:.2}%); test-only lines {}; mixed lines {}.",
        report["production"]["covered"],
        report["production"]["count"],
        report["production"]["percent"].as_f64().unwrap_or(0.0),
        report["testOnly"]["count"],
        report["mixed"]["count"],
    );
    let mut ranked: Vec<_> = report["files"].as_array().unwrap().iter().collect();
    ranked.sort_by_key(|file| std::cmp::Reverse(file["production"]["missed"].as_u64().unwrap()));
    let mut summary = String::from(
        "Production executable lines, using LLVM LCOV counts and parsed Rust test scopes.\n\
         Only #[cfg(test)]-required scopes and their module descendants are test-only.\n\
         Production failures/native/UI paths are retained. Mixed lines are reported separately.\n\
         This is Windows line coverage, not branch coverage or feature acceptance.\n\n",
    );
    summary.push_str(&format!(
        "Covered: {} / {} ({:.2}%)\nTest-only executable lines: {}\nMixed executable lines: {}\n\nLargest remaining production gaps:\n",
        report["production"]["covered"],
        report["production"]["count"],
        report["production"]["percent"].as_f64().unwrap_or(0.0),
        report["testOnly"]["count"], report["mixed"]["count"],
    ));
    for file in ranked
        .iter()
        .filter(|file| file["production"]["missed"] != 0)
        .take(25)
    {
        summary.push_str(&format!(
            "{:>5} missing / {:>5} executable  {}\n",
            file["production"]["missed"],
            file["production"]["count"],
            file["path"].as_str().unwrap(),
        ));
    }
    std::fs::write(run.join("production-summary.txt"), summary)?;
    Ok(())
}

// Evaluate only what is knowable when `test` is false. Other cfg predicates are
// unknown. In particular, any(test, windows) must remain production on Windows.
fn without_test(meta: &Meta) -> Option<bool> {
    match meta {
        Meta::Path(path) if path.is_ident("test") => Some(false),
        Meta::List(list) => {
            let args = list
                .parse_args_with(
                    syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
                )
                .ok()?;
            let values: Vec<_> = args.iter().map(without_test).collect();
            if list.path.is_ident("not") && values.len() == 1 {
                values[0].map(|value| !value)
            } else if list.path.is_ident("all") {
                if values.contains(&Some(false)) {
                    Some(false)
                } else if values.iter().all(|value| *value == Some(true)) {
                    Some(true)
                } else {
                    None
                }
            } else if list.path.is_ident("any") {
                if values.contains(&Some(true)) {
                    Some(true)
                } else if values.iter().all(|value| *value == Some(false)) {
                    Some(false)
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

fn test_attribute(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("test")
        || attr.path().is_ident("bench")
        || (attr.path().is_ident("cfg")
            && attr
                .parse_args::<Meta>()
                .is_ok_and(|meta| without_test(&meta) == Some(false)))
}

fn conditional_scope_attribute(attr: &syn::Attribute) -> bool {
    if !attr.path().is_ident("cfg_attr") {
        return false;
    }
    let Ok(args) =
        attr.parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
    else {
        return true;
    };
    args.iter().skip(1).any(|meta| {
        ["cfg", "test", "bench", "cfg_attr"]
            .iter()
            .any(|name| meta.path().is_ident(name))
    })
}

#[derive(Clone, Copy, Debug)]
struct Range {
    start: (usize, usize),
    end: (usize, usize),
}
impl From<Span> for Range {
    fn from(span: Span) -> Self {
        let start = span.start();
        let end = span.end();
        Self {
            start: (start.line, start.column),
            end: (end.line, end.column),
        }
    }
}

#[derive(Default, Debug)]
struct Scope {
    lines: Vec<String>,
    contexts: BTreeSet<bool>,
    ranges: Vec<Range>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum LineScope {
    Production,
    TestOnly,
    Mixed,
}

impl Scope {
    fn classify(&self, line: usize) -> LineScope {
        if self.contexts == BTreeSet::from([true]) {
            return LineScope::TestOnly;
        }
        let text = self
            .lines
            .get(line.saturating_sub(1))
            .map_or("", String::as_str);
        let mut mixed = false;
        for range in &self.ranges {
            if line < range.start.0 || line > range.end.0 {
                continue;
            }
            let prefix_ok = line != range.start.0
                || text
                    .get(..range.start.1)
                    .is_some_and(|prefix| prefix.trim().is_empty());
            let suffix_ok = line != range.end.0
                || text.get(range.end.1..).is_some_and(|suffix| {
                    let suffix = suffix
                        .trim_start()
                        .trim_start_matches([';', ','])
                        .trim_start();
                    suffix.is_empty() || suffix.starts_with("//")
                });
            if prefix_ok && suffix_ok {
                return LineScope::TestOnly;
            }
            mixed = true;
        }
        if mixed {
            LineScope::Mixed
        } else {
            LineScope::Production
        }
    }
}

struct Scanner {
    directory: PathBuf,
    stack: Vec<Span>,
    test_depth: usize,
    ranges: Vec<Range>,
    modules: Vec<(PathBuf, bool)>,
    errors: Vec<String>,
}

// These nodes own attributes. Keeping their full spans avoids excluding an
// enclosing production function when only one nested statement is test-only.
macro_rules! visit_scoped {
    ($method:ident, $node:ty) => {
        fn $method(&mut self, node: &'ast $node) {
            self.stack.push(node.span());
            syn::visit::$method(self, node);
            self.stack.pop();
        }
    };
}

impl<'ast> Visit<'ast> for Scanner {
    visit_scoped!(visit_expr, syn::Expr);
    visit_scoped!(visit_impl_item, syn::ImplItem);
    visit_scoped!(visit_trait_item, syn::TraitItem);
    visit_scoped!(visit_foreign_item, syn::ForeignItem);
    visit_scoped!(visit_local, syn::Local);
    visit_scoped!(visit_field, syn::Field);
    visit_scoped!(visit_field_value, syn::FieldValue);
    visit_scoped!(visit_fn_arg, syn::FnArg);
    visit_scoped!(visit_pat, syn::Pat);
    visit_scoped!(visit_generic_param, syn::GenericParam);
    visit_scoped!(visit_variant, syn::Variant);
    visit_scoped!(visit_arm, syn::Arm);
    visit_scoped!(visit_stmt_macro, syn::StmtMacro);

    fn visit_item(&mut self, item: &'ast syn::Item) {
        self.stack.push(item.span());
        syn::visit::visit_item(self, item);
        self.stack.pop();
    }

    fn visit_attribute(&mut self, attr: &'ast syn::Attribute) {
        if test_attribute(attr) {
            if let Some(span) = self.stack.last() {
                self.ranges.push((*span).into());
            } else {
                self.errors
                    .push("test attribute has no owning syntax node".into());
            }
        }
        if conditional_scope_attribute(attr) {
            self.errors
                .push("conditional test attributes require explicit scope support".into());
        }
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let test_only = item.attrs.iter().any(test_attribute);
        self.test_depth += usize::from(test_only);
        let old_directory = self.directory.clone();
        if item.attrs.iter().any(|attr| attr.path().is_ident("path")) {
            self.errors
                .push("#[path] module requires explicit scope support".into());
        } else if item.content.is_some() {
            self.directory.push(item.ident.to_string());
        } else {
            let base = self.directory.join(item.ident.to_string());
            let candidates = [base.with_extension("rs"), base.join("mod.rs")];
            let found: Vec<_> = candidates
                .into_iter()
                .filter(|path| path.is_file())
                .collect();
            if found.len() == 1 {
                self.modules.push((found[0].clone(), self.test_depth > 0));
            } else {
                self.errors.push(format!(
                    "cannot resolve module {} in {}",
                    item.ident,
                    self.directory.display()
                ));
            }
        }
        syn::visit::visit_item_mod(self, item);
        self.directory = old_directory;
        self.test_depth -= usize::from(test_only);
    }
}

fn source_scopes(root: &Path) -> Result<BTreeMap<PathBuf, Scope>> {
    let mut pending = vec![(root.to_owned(), false)];
    let mut files = BTreeMap::<PathBuf, Scope>::new();
    while let Some((file, inherited_test)) = pending.pop() {
        let file = file.canonicalize()?;
        let text = std::fs::read_to_string(&file)?;
        let ast = syn::parse_file(&text)?;
        let test_only = inherited_test || ast.attrs.iter().any(test_attribute);
        let scope = files.entry(file.clone()).or_default();
        if !scope.contexts.insert(test_only) {
            continue;
        }
        let stem = file.file_stem().unwrap().to_string_lossy();
        let directory = if stem == "main" || stem == "lib" || stem == "mod" {
            file.parent().unwrap().to_owned()
        } else {
            file.with_extension("")
        };
        let mut scanner = Scanner {
            directory,
            stack: Vec::new(),
            test_depth: usize::from(test_only),
            ranges: Vec::new(),
            modules: Vec::new(),
            errors: Vec::new(),
        };
        // File-level cfg(test) is already represented by the whole-file context.
        for item in &ast.items {
            scanner.visit_item(item);
        }
        if !scanner.errors.is_empty() {
            return Err(format!("{}: {}", file.display(), scanner.errors.join("; ")).into());
        }
        scope.lines = text.lines().map(str::to_owned).collect();
        scope.ranges = scanner.ranges;
        pending.extend(scanner.modules);
    }
    Ok(files)
}

type Counts = BTreeMap<PathBuf, BTreeMap<usize, u64>>;

#[derive(Debug)]
struct Lcov {
    counts: Counts,
    summaries: BTreeMap<PathBuf, (u64, u64)>,
}

fn parse_lcov(text: &str) -> Result<Lcov> {
    let mut result = Counts::new();
    let mut summaries = BTreeMap::new();
    let mut current = None;
    let (mut found, mut hit) = (None, None);
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("SF:") {
            let path = Path::new(path).canonicalize()?;
            if result.insert(path.clone(), BTreeMap::new()).is_some() {
                return Err(format!("duplicate LCOV file {}", path.display()).into());
            }
            current = Some(path);
            found = None;
            hit = None;
        } else if let Some(values) = line.strip_prefix("DA:") {
            let mut values = values.split(',');
            let number = values.next().ok_or("missing DA line")?.parse::<usize>()?;
            let count = values.next().ok_or("missing DA count")?.parse::<u64>()?;
            let file = current.as_ref().ok_or("DA without source file")?;
            if number == 0
                || result
                    .get_mut(file)
                    .unwrap()
                    .insert(number, count)
                    .is_some()
            {
                return Err("invalid or duplicate LCOV line".into());
            }
        } else if let Some(value) = line.strip_prefix("LF:") {
            found = Some(value.parse::<u64>()?);
        } else if let Some(value) = line.strip_prefix("LH:") {
            hit = Some(value.parse::<u64>()?);
        } else if line == "end_of_record" {
            let file = current.take().ok_or("end without source file")?;
            summaries.insert(
                file,
                (
                    found.ok_or("missing LCOV LF")?,
                    hit.ok_or("missing LCOV LH")?,
                ),
            );
        }
    }
    if result.is_empty() {
        return Err("empty LCOV report".into());
    }
    if current.is_some() {
        return Err("unfinished LCOV source record".into());
    }
    Ok(Lcov {
        counts: result,
        summaries,
    })
}

#[derive(Default)]
struct Tally {
    count: u64,
    covered: u64,
}
impl Tally {
    fn add(&mut self, hits: u64) {
        self.count += 1;
        self.covered += u64::from(hits > 0);
    }
    fn value(&self) -> Value {
        json!({"count": self.count, "covered": self.covered, "missed": self.count - self.covered,
            "percent": if self.count == 0 { 0.0 } else { self.covered as f64 / self.count as f64 * 100.0 }})
    }
}

fn partition(
    repo: &Path,
    scopes: &BTreeMap<PathBuf, Scope>,
    lcov: &Lcov,
    llvm: &Value,
) -> Result<Value> {
    let mut expected = BTreeMap::new();
    for unit in llvm["data"].as_array().ok_or("missing LLVM units")? {
        for file in unit["files"].as_array().ok_or("missing LLVM files")? {
            let path = Path::new(file["filename"].as_str().ok_or("missing LLVM filename")?)
                .canonicalize()?;
            let lines = &file["summary"]["lines"];
            expected.insert(
                path,
                (
                    lines["count"].as_u64().ok_or("missing LLVM line count")?,
                    lines["covered"]
                        .as_u64()
                        .ok_or("missing LLVM covered count")?,
                ),
            );
        }
    }
    if lcov.counts.keys().ne(expected.keys()) {
        return Err("LLVM JSON and LCOV file scopes differ".into());
    }
    let (mut production, mut test, mut mixed) =
        (Tally::default(), Tally::default(), Tally::default());
    let mut reports = Vec::new();
    for (file, lines) in &lcov.counts {
        let scope = scopes
            .get(file)
            .ok_or_else(|| format!("unclassified source file {}", file.display()))?;
        // LLVM's report/JSON/LF summaries sum function-level source lines; DA is
        // a merged file-line map. Nested functions/closures can overlap. Verify
        // the summary receipts agree, and label this distinct DA metric clearly.
        if expected[file] != lcov.summaries[file] {
            return Err(format!("LLVM line totals disagree for {}", file.display()).into());
        }
        let (mut file_production, mut file_test, mut file_mixed) =
            (Tally::default(), Tally::default(), Tally::default());
        let mut uncovered = Vec::new();
        let mut mixed_lines = Vec::new();
        for (line, hits) in lines {
            if *line > scope.lines.len() {
                return Err("LCOV references line beyond source".into());
            }
            match scope.classify(*line) {
                LineScope::Production => {
                    production.add(*hits);
                    file_production.add(*hits);
                    if *hits == 0 {
                        uncovered.push(*line);
                    }
                }
                LineScope::TestOnly => {
                    test.add(*hits);
                    file_test.add(*hits);
                }
                LineScope::Mixed => {
                    mixed.add(*hits);
                    file_mixed.add(*hits);
                    mixed_lines.push(*line);
                }
            }
        }
        reports.push(json!({"path": file.strip_prefix(repo)?.to_string_lossy().replace('\\', "/"),
            "production": file_production.value(), "testOnly": file_test.value(), "mixed": file_mixed.value(),
            "uncoveredProductionLines": uncovered, "mixedLines": mixed_lines}));
    }
    Ok(
        json!({"scope": "Windows unique file-line coverage from LLVM LCOV DA; parsed test-only scopes separated; no production exclusions",
        "production": production.value(), "testOnly": test.value(), "mixed": mixed.value(), "files": reports}),
    )
}

#[cfg(test)]
mod tests;
