//! Load one verified set of release WASMs, independently of the native test target.
use soroban_sdk::{Bytes, Env};
use std::{collections::BTreeMap, path::PathBuf, process::Command, sync::OnceLock};

pub(super) fn wasm(name: &str) -> &'static [u8] {
    static ARTIFACTS: OnceLock<BTreeMap<String, std::vec::Vec<u8>>> = OnceLock::new();
    ARTIFACTS
        .get_or_init(|| {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let script = root.join("scripts/wasm_artifacts.py");
            let directory = std::env::var_os("CROSSING_WASM_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    let output = Command::new("python3")
                        .arg(&script)
                        .arg("directory")
                        .output()
                        .expect("Resolve Cargo's WASM target directory");
                    assert!(output.status.success(), "{:?}", output);
                    PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
                });
            let output = Command::new("python3")
                .arg(script)
                .arg("verify")
                .env("CROSSING_WASM_DIR", &directory)
                .output()
                .expect("Verify WASM build inputs and artifact hashes");
            assert!(
                output.status.success(),
                "WASM verification failed; run python3 scripts/wasm_artifacts.py build:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let manifest: serde_json::Value = serde_json::from_slice(
                &std::fs::read(directory.join("test-wasm-manifest.json")).unwrap(),
            )
            .unwrap();
            let env = Env::default();
            manifest["artifacts"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(file, record)| {
                    let bytes = std::fs::read(directory.join(file)).unwrap();
                    // Hash the loaded bytes too, so a replacement after the Python
                    // verification cannot silently change what the tests execute.
                    let digest: String = env
                        .crypto()
                        .sha256(&Bytes::from_slice(&env, &bytes))
                        .to_array()
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect();
                    assert_eq!(record["sha256"].as_str(), Some(digest.as_str()), "{file}");
                    (file.trim_end_matches(".wasm").to_owned(), bytes)
                })
                .collect()
        })
        .get(name)
        .unwrap_or_else(|| panic!("Unknown WASM contract: {name}"))
}
