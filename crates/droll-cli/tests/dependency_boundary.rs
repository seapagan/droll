use std::{collections::BTreeSet, path::PathBuf};

use cargo_metadata::{DependencyKind, Metadata, MetadataCommand, Package, PackageId};

const GUI_PACKAGE_NAMES: &[&str] = &["avian3d", "droll-gui", "raw-window-handle"];
const GUI_PACKAGE_PREFIXES: &[&str] = &["bevy", "wgpu", "winit"];

fn workspace_metadata() -> Metadata {
    let workspace_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("Cargo.toml");

    MetadataCommand::new()
        .manifest_path(workspace_manifest)
        .other_options(vec!["--locked".to_owned()])
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

fn is_production_dependency(kind: &DependencyKind) -> bool {
    matches!(kind, DependencyKind::Normal | DependencyKind::Build)
}

fn production_dependency_closure(metadata: &Metadata, root: &PackageId) -> BTreeSet<String> {
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
            let is_production = dependency
                .dep_kinds
                .iter()
                .any(|kind| is_production_dependency(&kind.kind));
            if is_production && visited.insert(dependency.pkg.clone()) {
                pending.push(&dependency.pkg);
            }
        }
    }

    visited
        .iter()
        .map(|package_id| package_by_id(metadata, package_id).name.to_string())
        .collect()
}

fn is_gui_package(package_name: &str) -> bool {
    GUI_PACKAGE_NAMES.contains(&package_name)
        || GUI_PACKAGE_PREFIXES.iter().any(|prefix| {
            package_name == *prefix
                || package_name.starts_with(&format!("{prefix}-"))
                || package_name.starts_with(&format!("{prefix}_"))
        })
}

#[test]
fn test_production_dependency_kinds_exclude_development_dependencies() {
    assert!(is_production_dependency(&DependencyKind::Normal));
    assert!(is_production_dependency(&DependencyKind::Build));
    assert!(!is_production_dependency(&DependencyKind::Development));
}

#[test]
fn test_gui_package_classification() {
    for package_name in ["avian3d", "bevy", "bevy_render", "wgpu-core", "winit"] {
        assert!(is_gui_package(package_name), "{package_name} should be GUI");
    }

    for package_name in ["aviary", "bevyish", "wgpuish", "winitializer"] {
        assert!(
            !is_gui_package(package_name),
            "{package_name} should not be GUI"
        );
    }
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

    for package in [core, cli] {
        let gui_dependencies: BTreeSet<_> = production_dependency_closure(&metadata, &package.id)
            .into_iter()
            .filter(|package_name| is_gui_package(package_name))
            .collect();
        assert!(
            gui_dependencies.is_empty(),
            "{} normal/build dependency closure must exclude GUI packages: {gui_dependencies:?}",
            package.name
        );
    }
}
