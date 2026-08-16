use std::{collections::BTreeSet, path::PathBuf};

use cargo_metadata::{DependencyKind, Metadata, MetadataCommand, Package, PackageId};

const GUI_PACKAGES: &[&str] = &["avian3d", "bevy", "droll-gui", "wgpu", "winit"];

fn workspace_metadata() -> Metadata {
    let workspace_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("Cargo.toml");

    MetadataCommand::new()
        .manifest_path(workspace_manifest)
        .exec()
        .expect("workspace metadata should resolve")
}

fn package<'a>(metadata: &'a Metadata, name: &str) -> &'a Package {
    metadata
        .packages
        .iter()
        .find(|package| package.name == name)
        .unwrap_or_else(|| panic!("package {name} should exist"))
}

fn package_by_id<'a>(metadata: &'a Metadata, package_id: &PackageId) -> &'a Package {
    metadata
        .packages
        .iter()
        .find(|package| &package.id == package_id)
        .expect("resolved package should exist in metadata")
}

fn direct_normal_dependencies(package: &Package) -> BTreeSet<&str> {
    package
        .dependencies
        .iter()
        .filter(|dependency| dependency.kind == DependencyKind::Normal)
        .map(|dependency| dependency.name.as_str())
        .collect()
}

fn normal_dependency_closure(metadata: &Metadata, root: &PackageId) -> BTreeSet<String> {
    let resolve = metadata
        .resolve
        .as_ref()
        .expect("metadata should include a resolve graph");
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();

    while let Some(package_id) = pending.pop() {
        let node = resolve
            .nodes
            .iter()
            .find(|node| &node.id == package_id)
            .expect("resolved package should have a node");

        for dependency in &node.deps {
            let is_normal = dependency
                .dep_kinds
                .iter()
                .any(|kind| kind.kind == DependencyKind::Normal);
            if is_normal && visited.insert(dependency.pkg.clone()) {
                pending.push(&dependency.pkg);
            }
        }
    }

    visited
        .iter()
        .map(|package_id| package_by_id(metadata, package_id).name.to_string())
        .collect()
}

#[test]
fn test_workspace_dependency_boundaries() {
    let metadata = workspace_metadata();
    let core = package(&metadata, "droll-core");
    let cli = package(&metadata, "droll-cli");
    let gui = package(&metadata, "droll-gui");

    assert!(direct_normal_dependencies(core).is_empty());
    assert_eq!(
        direct_normal_dependencies(cli),
        BTreeSet::from(["droll-core"])
    );
    assert_eq!(
        direct_normal_dependencies(gui),
        BTreeSet::from(["avian3d", "bevy", "droll-core"])
    );

    let cli_closure = normal_dependency_closure(&metadata, &cli.id);
    for package_name in GUI_PACKAGES {
        assert!(
            !cli_closure.contains(*package_name),
            "CLI dependency closure must exclude {package_name}"
        );
    }
}
