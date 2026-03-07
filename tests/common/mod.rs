#![allow(dead_code)]

use std::fs;

use tempfile::TempDir;

pub fn setup_project_root() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create a global config
    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\n", root.display()),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), "example.com\n").unwrap();

    // example.com/team/project-alpha/.git
    fs::create_dir_all(root.join("example.com/team/project-alpha/.git")).unwrap();
    // example.com/team/project-beta/.git
    fs::create_dir_all(root.join("example.com/team/project-beta/.git")).unwrap();

    tmp
}

pub fn setup_project_root_with_config() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\n", root.display()),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), "example.com\n").unwrap();

    // Domain config
    let domain_dir = root.join("example.com");
    fs::create_dir_all(&domain_dir).unwrap();
    fs::write(
        domain_dir.join(".git-jump.toml"),
        "[git_config]\n\"user.name\" = \"Test User\"\n\n[hooks]\non_enter = [\"echo domain-hook\"]\n",
    )
    .unwrap();

    // Group dir
    let group_dir = domain_dir.join("team");
    fs::create_dir_all(&group_dir).unwrap();
    fs::write(
        group_dir.join(".git-jump.toml"),
        "[env]\nGOPATH = \"/go\"\n",
    )
    .unwrap();

    // Project with hooks
    let project_dir = group_dir.join("project-alpha");
    fs::create_dir_all(project_dir.join(".git")).unwrap();
    fs::write(
        project_dir.join(".git-jump.toml"),
        "[hooks]\non_enter = [\"echo project-hook\"]\n",
    )
    .unwrap();

    tmp
}
