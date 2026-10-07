use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// What kind of project was picked — drives which IDE the UI recommends
/// preinstalling on the VM.
#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ProjectKind {
    Flutter,
    ReactNative,
    Android,
    General,
}

/// An IDE the Ansible playbook can preinstall.
#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Ide {
    Vscode,
    AndroidStudio,
}

/// Repo-relative files that are enough to tell the project kinds apart.
/// Kept to a fixed, small list so a GitHub repo can be classified with a
/// handful of contents-API calls instead of a full clone or tree walk.
pub const MARKER_FILES: &[&str] = &[
    "pubspec.yaml",
    "package.json",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "gradle/libs.versions.toml",
    "app/build.gradle",
    "app/build.gradle.kts",
];

/// Classifies from the contents of whichever `MARKER_FILES` exist. Order
/// matters: Flutter and React Native projects both carry an `android/`
/// Gradle project, so they're checked before plain Android.
pub fn classify(files: &HashMap<&str, String>) -> ProjectKind {
    if files
        .get("pubspec.yaml")
        .is_some_and(|s| s.contains("sdk: flutter"))
    {
        return ProjectKind::Flutter;
    }

    if let Some(pkg) = files.get("package.json") {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(pkg) {
            let has_dep = |name: &str| {
                ["dependencies", "devDependencies"]
                    .iter()
                    .any(|section| json[section].get(name).is_some())
            };
            if has_dep("react-native") || has_dep("expo") {
                return ProjectKind::ReactNative;
            }
        }
    }

    let is_android = files
        .iter()
        .any(|(name, content)| name.contains("gradle") && content.contains("com.android"));
    if is_android {
        return ProjectKind::Android;
    }

    ProjectKind::General
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(files: &[(&'static str, &str)]) -> ProjectKind {
        classify(&files.iter().map(|(n, c)| (*n, c.to_string())).collect())
    }

    #[test]
    fn detects_flutter() {
        let pubspec = "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n";
        assert_eq!(kind(&[("pubspec.yaml", pubspec)]), ProjectKind::Flutter);
    }

    #[test]
    fn plain_dart_package_is_not_flutter() {
        assert_eq!(
            kind(&[("pubspec.yaml", "name: lib\ndependencies:\n  http: ^1.0.0\n")]),
            ProjectKind::General
        );
    }

    #[test]
    fn detects_react_native_and_expo() {
        let rn = r#"{"dependencies": {"react": "19.0.0", "react-native": "0.80.0"}}"#;
        assert_eq!(kind(&[("package.json", rn)]), ProjectKind::ReactNative);
        let expo = r#"{"dependencies": {"expo": "~54.0.0"}}"#;
        assert_eq!(kind(&[("package.json", expo)]), ProjectKind::ReactNative);
    }

    #[test]
    fn plain_web_app_is_general() {
        let web = r#"{"dependencies": {"react": "19.0.0", "react-dom": "19.0.0"}}"#;
        assert_eq!(kind(&[("package.json", web)]), ProjectKind::General);
    }

    #[test]
    fn detects_android_from_version_catalog_or_gradle() {
        let toml = "[plugins]\nandroid-application = { id = \"com.android.application\" }\n";
        assert_eq!(
            kind(&[("gradle/libs.versions.toml", toml)]),
            ProjectKind::Android
        );
        let gradle = "plugins { id 'com.android.application' }";
        assert_eq!(kind(&[("app/build.gradle", gradle)]), ProjectKind::Android);
    }

    #[test]
    fn plain_jvm_gradle_project_is_general() {
        assert_eq!(
            kind(&[("build.gradle.kts", "plugins { kotlin(\"jvm\") }")]),
            ProjectKind::General
        );
    }

    #[test]
    fn empty_is_general() {
        assert_eq!(kind(&[]), ProjectKind::General);
    }
}
