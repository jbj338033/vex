use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodePackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeFramework {
    Next,
    Vite,
    Remix,
    Plain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PythonPackageManager {
    Uv,
    Poetry,
    Pip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JvmBuildTool {
    Gradle,
    Maven,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JvmLanguage {
    Java,
    Kotlin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectType {
    Dockerfile,
    Node {
        package_manager: NodePackageManager,
        framework: NodeFramework,
    },
    Python {
        package_manager: PythonPackageManager,
    },
    Go,
    Rust,
    SpringBoot {
        build_tool: JvmBuildTool,
        language: JvmLanguage,
    },
    Static,
}

pub fn detect(dir: &Path) -> Option<ProjectType> {
    if dir.join("Dockerfile").exists() {
        return Some(ProjectType::Dockerfile);
    }

    if dir.join("package.json").exists() {
        return Some(detect_node(dir));
    }

    if dir.join("pyproject.toml").exists() || dir.join("requirements.txt").exists() {
        return Some(detect_python(dir));
    }

    if dir.join("go.mod").exists() {
        return Some(ProjectType::Go);
    }

    if dir.join("Cargo.toml").exists() {
        return Some(ProjectType::Rust);
    }

    if let Some(spring) = detect_spring_boot(dir) {
        return Some(spring);
    }

    if dir.join("index.html").exists() {
        return Some(ProjectType::Static);
    }

    None
}

fn detect_node(dir: &Path) -> ProjectType {
    let package_manager = if dir.join("pnpm-lock.yaml").exists() {
        NodePackageManager::Pnpm
    } else if dir.join("yarn.lock").exists() {
        NodePackageManager::Yarn
    } else if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
        NodePackageManager::Bun
    } else {
        NodePackageManager::Npm
    };

    let framework = detect_node_framework(dir);

    ProjectType::Node {
        package_manager,
        framework,
    }
}

fn detect_node_framework(dir: &Path) -> NodeFramework {
    if dir.join("next.config.js").exists()
        || dir.join("next.config.mjs").exists()
        || dir.join("next.config.ts").exists()
    {
        return NodeFramework::Next;
    }

    if dir.join("vite.config.js").exists()
        || dir.join("vite.config.ts").exists()
        || dir.join("vite.config.mjs").exists()
    {
        return NodeFramework::Vite;
    }

    if dir.join("remix.config.js").exists() || dir.join("remix.config.mjs").exists() {
        return NodeFramework::Remix;
    }

    NodeFramework::Plain
}

fn detect_spring_boot(dir: &Path) -> Option<ProjectType> {
    let build_tool = if dir.join("build.gradle.kts").exists() || dir.join("build.gradle").exists() {
        JvmBuildTool::Gradle
    } else if dir.join("pom.xml").exists() {
        JvmBuildTool::Maven
    } else {
        return None;
    };

    let language = if dir.join("src/main/kotlin").exists() {
        JvmLanguage::Kotlin
    } else {
        JvmLanguage::Java
    };

    Some(ProjectType::SpringBoot {
        build_tool,
        language,
    })
}

fn detect_python(dir: &Path) -> ProjectType {
    let package_manager = if dir.join("uv.lock").exists() {
        PythonPackageManager::Uv
    } else if dir.join("poetry.lock").exists() {
        PythonPackageManager::Poetry
    } else {
        PythonPackageManager::Pip
    };

    ProjectType::Python { package_manager }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn detects_dockerfile() {
        let dir = setup();
        fs::write(dir.path().join("Dockerfile"), "FROM node").unwrap();
        assert_eq!(detect(dir.path()), Some(ProjectType::Dockerfile));
    }

    #[test]
    fn dockerfile_takes_priority() {
        let dir = setup();
        fs::write(dir.path().join("Dockerfile"), "FROM node").unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert_eq!(detect(dir.path()), Some(ProjectType::Dockerfile));
    }

    #[test]
    fn detects_nodejs_with_pnpm() {
        let dir = setup();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();
        assert_eq!(
            detect(dir.path()),
            Some(ProjectType::Node {
                package_manager: NodePackageManager::Pnpm,
                framework: NodeFramework::Plain,
            })
        );
    }

    #[test]
    fn detects_nodejs_with_next() {
        let dir = setup();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::write(dir.path().join("next.config.mjs"), "").unwrap();
        assert_eq!(
            detect(dir.path()),
            Some(ProjectType::Node {
                package_manager: NodePackageManager::Npm,
                framework: NodeFramework::Next,
            })
        );
    }

    #[test]
    fn detects_nodejs_with_vite_and_yarn() {
        let dir = setup();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::write(dir.path().join("yarn.lock"), "").unwrap();
        fs::write(dir.path().join("vite.config.ts"), "").unwrap();
        assert_eq!(
            detect(dir.path()),
            Some(ProjectType::Node {
                package_manager: NodePackageManager::Yarn,
                framework: NodeFramework::Vite,
            })
        );
    }

    #[test]
    fn detects_python_with_uv() {
        let dir = setup();
        fs::write(dir.path().join("pyproject.toml"), "").unwrap();
        fs::write(dir.path().join("uv.lock"), "").unwrap();
        assert_eq!(
            detect(dir.path()),
            Some(ProjectType::Python {
                package_manager: PythonPackageManager::Uv,
            })
        );
    }

    #[test]
    fn detects_python_with_pip() {
        let dir = setup();
        fs::write(dir.path().join("requirements.txt"), "flask").unwrap();
        assert_eq!(
            detect(dir.path()),
            Some(ProjectType::Python {
                package_manager: PythonPackageManager::Pip,
            })
        );
    }

    #[test]
    fn detects_go() {
        let dir = setup();
        fs::write(dir.path().join("go.mod"), "module example").unwrap();
        assert_eq!(detect(dir.path()), Some(ProjectType::Go));
    }

    #[test]
    fn detects_rust() {
        let dir = setup();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        assert_eq!(detect(dir.path()), Some(ProjectType::Rust));
    }

    #[test]
    fn detects_static() {
        let dir = setup();
        fs::write(dir.path().join("index.html"), "<html>").unwrap();
        assert_eq!(detect(dir.path()), Some(ProjectType::Static));
    }

    #[test]
    fn detects_spring_boot_gradle_kotlin() {
        let dir = setup();
        fs::write(dir.path().join("build.gradle.kts"), "").unwrap();
        fs::create_dir_all(dir.path().join("src/main/kotlin")).unwrap();
        assert_eq!(
            detect(dir.path()),
            Some(ProjectType::SpringBoot {
                build_tool: JvmBuildTool::Gradle,
                language: JvmLanguage::Kotlin,
            })
        );
    }

    #[test]
    fn detects_spring_boot_gradle_java() {
        let dir = setup();
        fs::write(dir.path().join("build.gradle.kts"), "").unwrap();
        fs::create_dir_all(dir.path().join("src/main/java")).unwrap();
        assert_eq!(
            detect(dir.path()),
            Some(ProjectType::SpringBoot {
                build_tool: JvmBuildTool::Gradle,
                language: JvmLanguage::Java,
            })
        );
    }

    #[test]
    fn detects_spring_boot_maven() {
        let dir = setup();
        fs::write(dir.path().join("pom.xml"), "").unwrap();
        fs::create_dir_all(dir.path().join("src/main/java")).unwrap();
        assert_eq!(
            detect(dir.path()),
            Some(ProjectType::SpringBoot {
                build_tool: JvmBuildTool::Maven,
                language: JvmLanguage::Java,
            })
        );
    }

    #[test]
    fn returns_none_for_empty_dir() {
        let dir = setup();
        assert_eq!(detect(dir.path()), None);
    }
}
