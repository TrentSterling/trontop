use super::super::maintenance::{powershell, powershell_path};
use super::*;

// Package parameters are passed as data in the child's environment. PowerShell only evaluates
// these fixed scripts. Main packages are scoped to the current user.
const LIST: &str = r#"Import-Module Appx -ErrorAction Stop
$rows = @(Get-AppxPackage -PackageTypeFilter Main -ErrorAction Stop | Select-Object Name,PackageFullName,Publisher,Version,InstallLocation,NonRemovable,IsFramework,@{n='SystemSigned';e={$_.SignatureKind -eq 'System'}})
ConvertTo-Json -InputObject $rows -Compress"#;
const REMOVE: &str = r#"Import-Module Appx -ErrorAction Stop
$matches = @(Get-AppxPackage -PackageTypeFilter Main -ErrorAction Stop | Where-Object { $_.PackageFullName -ceq $env:TRONTOP_PACKAGE_FULL_NAME })
if ($matches.Count -ne 1 -or $matches[0].NonRemovable -ne $false -or $matches[0].IsFramework -ne $false -or $matches[0].SignatureKind -eq 'System') { throw 'Package changed or is managed by Windows.' }
Remove-AppxPackage -Package $matches[0].PackageFullName -ErrorAction Stop"#;
fn parse(json: &str) -> io::Result<Inventory> {
    let rows: Vec<serde_json::Value> = serde_json::from_str(json).map_err(io::Error::other)?;
    let engine = powershell_path()?;
    let mut apps = Inventory {
        packaged: true,
        ..Default::default()
    };
    for row in rows {
        let text = |key: &str| row[key].as_str().unwrap_or_default().to_owned();
        let key = text("PackageFullName");
        let name = text("Name");
        if key.is_empty() || name.is_empty() {
            apps.issues += 1;
            continue;
        }
        let can_remove = ["NonRemovable", "IsFramework", "SystemSigned"]
            .iter()
            .all(|&k| row[k].as_bool() == Some(false));
        apps.rows.push(App {
            hive: Hive::CurrentUser,
            packaged: true,
            key: key.clone(),
            name,
            publisher: text("Publisher"),
            version: text("Version"),
            location: local_path(&text("InstallLocation")),
            size: None,
            uninstall: can_remove.then(|| Launch {
                label: "Uninstall",
                exe: engine.clone(),
                arguments: key,
            }),
            maintenance: None,
        });
    }
    apps.rows.sort_by_cached_key(|a| a.name.to_lowercase());
    Ok(apps)
}
pub(super) fn inventory() -> io::Result<Inventory> {
    parse(&powershell(LIST, None)?)
}
pub(super) fn remove(app: &App) -> io::Result<String> {
    powershell(REMOVE, Some(("TRONTOP_PACKAGE_FULL_NAME", &app.key)))?;
    Ok(format!(
        "Windows removed {} for the current user. Refresh the package list.",
        app.name
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "read-only Windows package inventory"]
    fn live_packaged_app_counts_only() {
        let data = inventory().unwrap();
        println!(
            "{} current-user main packages, {} removable",
            data.rows.len(),
            data.rows.iter().filter(|a| a.uninstall.is_some()).count()
        );
    }
    #[test]
    fn protected_and_incomplete_package_metadata_never_enable_removal() {
        let data=parse(r#"[{"Name":"A","PackageFullName":"A_1","NonRemovable":false,"IsFramework":false,"SystemSigned":false},{"Name":"B","PackageFullName":"B_1","NonRemovable":true,"IsFramework":false,"SystemSigned":false},{"Name":"C","PackageFullName":"C_1"}]"#).unwrap();
        assert_eq!(data.rows.len(), 3);
        assert!(data.rows[0].uninstall.is_some());
        assert!(data.rows[1..].iter().all(|a| a.uninstall.is_none()));
    }
}
