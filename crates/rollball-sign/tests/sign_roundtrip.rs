//! Sign + Verify roundtrip integration test

use std::fs;
use std::io::Write;
use std::path::Path;

fn create_test_agent_zip(path: &Path) {
    let file = fs::File::create(path).expect("Failed to create test ZIP");
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    writer.start_file("manifest.toml", options).expect("Failed to add manifest.toml");
    writer.write_all(br#"agent_id = "com.test.sign"
version = "1.0.0"
name = "Sign Test Agent"
description = "Test agent for signing"
author = "test"
runtime_version = "0.1.0"

[llm]
provider = "openai"
model = "gpt-4"
"#).expect("Failed to write manifest.toml");

    writer.start_file("prompts/system.md", options).expect("Failed to add system.md");
    writer.write_all(b"You are a test agent.").expect("Failed to write system.md");
    writer.finish().expect("Failed to finish ZIP");
}

#[test]
fn test_sign_and_verify_roundtrip() {
    let temp_dir = std::env::temp_dir().join("rollball-test-sign-roundtrip");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    let key_dir = temp_dir.join("keys");
    let unsigned_path = temp_dir.join("test.unsigned.agent");
    let signed_path = temp_dir.join("test.signed.agent");

    let _keypair = rollball_sign::keygen::generate_and_save(
        &key_dir,
        rollball_sign::keygen::KeyType::Developer,
    ).expect("Failed to generate keypair");

    assert!(key_dir.join("developer.key").exists());
    assert!(key_dir.join("developer.pub").exists());

    create_test_agent_zip(&unsigned_path);
    assert!(unsigned_path.exists());

    rollball_sign::sign::sign_package(
        &unsigned_path,
        &signed_path,
        &key_dir,
        rollball_sign::keygen::KeyType::Developer,
    ).expect("Failed to sign package");

    assert!(signed_path.exists(), "Signed package should exist");

    let result = rollball_sign::verify::verify_package(&signed_path)
        .expect("Failed to verify package");
    assert!(result.valid, "Signature should be valid");

    let unsigned_result = rollball_sign::verify::verify_package(&unsigned_path);
    assert!(unsigned_result.is_err(), "Unsigned package should fail verification");
}

/// Test signing and verifying the actual weather agent package
/// This validates S4 task 5.1.2: weather-agent package signing
#[test]
fn test_weather_agent_sign_and_verify() {
    let temp_dir = std::env::temp_dir().join("rollball-test-weather-sign");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    let key_dir = temp_dir.join("keys");
    let unsigned_path = temp_dir.join("weather.unsigned.agent");
    let signed_path = temp_dir.join("weather.signed.agent");

    // Step 1: Generate developer keypair
    let _keypair = rollball_sign::keygen::generate_and_save(
        &key_dir,
        rollball_sign::keygen::KeyType::Developer,
    ).expect("Failed to generate keypair");

    // Step 2: Create weather agent ZIP with proper manifest
    let file = fs::File::create(&unsigned_path).expect("Failed to create weather ZIP");
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    let manifest_toml = r#"agent_id = "com.example.weather"
version = "1.0.0"
name = "Weather Agent"
description = "A weather query agent that remembers your city preference"
author = "Rollball Team"
runtime_version = "0.1.0"

[[permissions]]
type = "Network"
value = "https://wttr.in"

[[permissions]]
type = "MemoryRead"

[[permissions]]
type = "MemoryWrite"

[[tools]]
name = "http_request"

[[tools]]
name = "memory_store"

[[tools]]
name = "memory_recall"

[llm]
provider = "openai"
model = "gpt-4"
temperature = 0.7
"#;

    writer.start_file("manifest.toml", options).expect("Failed to add manifest.toml");
    writer.write_all(manifest_toml.as_bytes()).expect("Failed to write manifest.toml");

    writer.start_file("prompts/system.md", options).expect("Failed to add system.md");
    writer.write_all(b"You are a helpful weather assistant. You can query weather using the http_request tool (GET https://wttr.in/{city}?format=3). Remember user city preferences with memory tools.").expect("Failed to write system.md");

    writer.start_file("prompts/default.md", options).expect("Failed to add default.md");
    writer.write_all(b"How can I help you with weather today?").expect("Failed to write default.md");

    writer.finish().expect("Failed to finish ZIP");

    // Step 3: Sign the package
    rollball_sign::sign::sign_package(
        &unsigned_path,
        &signed_path,
        &key_dir,
        rollball_sign::keygen::KeyType::Developer,
    ).expect("Failed to sign weather agent package");

    assert!(signed_path.exists(), "Signed package should exist");

    // Step 4: Verify the signed package
    let result = rollball_sign::verify::verify_package(&signed_path)
        .expect("Failed to verify weather agent package");
    assert!(result.valid, "Weather agent signature should be valid");
    assert_eq!(result.signer, "developer", "Should be signed by developer key");
    // Sections: manifest.toml, prompts/system.md, prompts/default.md, META-INF/SIGNING.BLOCK
    assert!(result.sections_count >= 3, "Should have at least 3 content sections, got {}", result.sections_count);

    // Step 5: Verify unsigned package fails
    let unsigned_result = rollball_sign::verify::verify_package(&unsigned_path);
    assert!(unsigned_result.is_err(), "Unsigned weather agent should fail verification");

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}
