use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/coverage/scope-fixtures")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn write(&self, path: &str, text: &str) -> PathBuf {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path.canonicalize().unwrap()
    }
}

#[test]
fn cfg_logic_does_not_exclude_alternative_production_builds() {
    for cfg in [
        "test",
        "all(windows, test)",
        "not(not(test))",
        "any(test, all(test, unix))",
    ] {
        assert_eq!(
            without_test(&syn::parse_str::<Meta>(cfg).unwrap()),
            Some(false),
            "{cfg}"
        );
    }
    for cfg in [
        "windows",
        "any(test, windows)",
        "not(windows)",
        "all(windows, not(test))",
    ] {
        assert_eq!(
            without_test(&syn::parse_str::<Meta>(cfg).unwrap()),
            None,
            "{cfg}"
        );
    }
    assert_eq!(
        without_test(&syn::parse_str::<Meta>("not(test)").unwrap()),
        Some(true)
    );
}

#[test]
fn conditional_lint_attributes_do_not_change_the_source_scope() {
    let item: syn::ItemFn =
        syn::parse_str("#[cfg_attr(test, allow(dead_code))] fn production() {}").unwrap();
    assert!(!conditional_scope_attribute(&item.attrs[0]));
    let item: syn::ItemFn =
        syn::parse_str("#[cfg_attr(windows, cfg(test))] fn conditional() {}").unwrap();
    assert!(conditional_scope_attribute(&item.attrs[0]));
}

#[test]
fn test_modules_propagate_to_nested_files_without_filename_heuristics() {
    let fixture = Fixture::new();
    let root = fixture.write("src/main.rs", "mod contest;\n#[cfg(test)]\nmod helpers;\n");
    let production = fixture.write("src/contest.rs", "pub fn latest() {}\n");
    let helpers = fixture.write("src/helpers.rs", "mod nested;\npub fn fixture() {}\n");
    let nested = fixture.write("src/helpers/nested.rs", "pub fn helper() {}\n");
    let scopes = source_scopes(&root).unwrap();
    assert_eq!(scopes[&production].classify(1), LineScope::Production);
    assert_eq!(scopes[&helpers].classify(2), LineScope::TestOnly);
    assert_eq!(scopes[&nested].classify(1), LineScope::TestOnly);
}

#[test]
fn inline_tests_and_test_statements_keep_the_surrounding_function() {
    let fixture = Fixture::new();
    let root = fixture.write(
        "src/main.rs",
        concat!(
            "fn production() {\n",       // 1
            "    let before = 1;\n",     // 2
            "    #[cfg(test)]\n",        // 3
            "    track();\n",            // 4
            "    let after = 2;\n",      // 5
            "}\n",                       // 6
            "#[cfg(test)]\n",            // 7
            "mod tests {\n",             // 8
            "    fn regression() {}\n",  // 9
            "}\n",                       // 10
            "#[test]\n",                 // 11
            "fn bare_test() {}\n",       // 12
            "fn never_exercised() {}\n", // 13
        ),
    );
    let scopes = source_scopes(&root).unwrap();
    let scope = &scopes[&root];
    for line in [1, 2, 5, 6, 13] {
        assert_eq!(scope.classify(line), LineScope::Production, "{line}");
    }
    for line in [3, 4, 7, 8, 9, 10, 11, 12] {
        assert_eq!(scope.classify(line), LineScope::TestOnly, "{line}");
    }
}

#[test]
fn partial_lines_are_reported_as_mixed_instead_of_disappearing() {
    let fixture = Fixture::new();
    let root = fixture.write(
        "src/main.rs",
        "fn production() { #[cfg(test)] track(); execute(); }\n",
    );
    assert_eq!(
        source_scopes(&root).unwrap()[&root].classify(1),
        LineScope::Mixed
    );
}

#[test]
fn test_field_initializers_do_not_exclude_production_struct_fields() {
    let fixture = Fixture::new();
    let root = fixture.write(
        "src/main.rs",
        concat!(
            "fn production() {\n",                // 1
            "    let state = State {\n",          // 2
            "        worker: create_worker(),\n", // 3
            "        #[cfg(test)]\n",             // 4
            "        observed: track(),\n",       // 5
            "    };\n",                           // 6
            "}\n",                                // 7
        ),
    );
    let scopes = source_scopes(&root).unwrap();
    for line in [1, 2, 3, 6, 7] {
        assert_eq!(
            scopes[&root].classify(line),
            LineScope::Production,
            "{line}"
        );
    }
    for line in [4, 5] {
        assert_eq!(scopes[&root].classify(line), LineScope::TestOnly, "{line}");
    }
}

#[test]
fn nested_inline_test_modules_and_impl_methods_use_their_own_scopes() {
    let fixture = Fixture::new();
    let root = fixture.write(
        "src/main.rs",
        concat!(
            "struct App;\n",
            "impl App {\n",
            "    #[cfg(test)]\n",
            "    fn fake() {}\n",
            "    fn actual() {}\n",
            "}\n",
            "mod outer {\n",
            "    #[cfg(all(test, windows))]\n",
            "    mod fixtures { mod nested; }\n",
            "}\n",
        ),
    );
    let nested = fixture.write("src/outer/fixtures/nested.rs", "fn fixture() {}\n");
    let scopes = source_scopes(&root).unwrap();
    assert_eq!(scopes[&root].classify(4), LineScope::TestOnly);
    assert_eq!(scopes[&root].classify(5), LineScope::Production);
    assert_eq!(scopes[&nested].classify(1), LineScope::TestOnly);
}

#[test]
fn unsupported_module_paths_fail_measurement_instead_of_guessing() {
    let fixture = Fixture::new();
    let root = fixture.write("src/main.rs", "#[path = \"elsewhere.rs\"] mod tests;\n");
    assert!(
        source_scopes(&root)
            .unwrap_err()
            .to_string()
            .contains("#[path]")
    );
}

#[test]
fn partition_retains_uncovered_production_and_checks_llvm_totals() {
    let fixture = Fixture::new();
    let root = fixture.write(
        "src/main.rs",
        "fn exercised() {}\nfn untested() {}\n#[test]\nfn regression() {}\n",
    );
    let scopes = source_scopes(&root).unwrap();
    let lcov = format!(
        "SF:{}\nDA:1,4\nDA:2,0\nDA:4,1\nLF:3\nLH:2\nend_of_record\n",
        root.display()
    );
    let counts = parse_lcov(&lcov).unwrap();
    let mut llvm = json!({"data": [{"files": [{"filename": root, "summary": {"lines": {"count": 3, "covered": 2}}}]}]});
    let report = partition(&fixture.0, &scopes, &counts, &llvm).unwrap();
    assert_eq!(report["production"]["count"], 2);
    assert_eq!(report["production"]["covered"], 1);
    assert_eq!(report["production"]["percent"], 50.0);
    assert_eq!(report["testOnly"]["count"], 1);
    assert_eq!(report["files"][0]["uncoveredProductionLines"], json!([2]));
    llvm["data"][0]["files"][0]["summary"]["lines"]["covered"] = json!(3);
    assert!(
        partition(&fixture.0, &scopes, &counts, &llvm)
            .unwrap_err()
            .to_string()
            .contains("totals disagree")
    );
}

#[test]
fn duplicate_lcov_lines_are_rejected_instead_of_masking_counts() {
    let fixture = Fixture::new();
    let root = fixture.write("src/main.rs", "fn main() {}\n");
    let lcov = format!(
        "SF:{}\nDA:1,0\nDA:1,1\nLF:1\nLH:1\nend_of_record\n",
        root.display()
    );
    assert!(
        parse_lcov(&lcov)
            .unwrap_err()
            .to_string()
            .contains("duplicate LCOV line")
    );
}
