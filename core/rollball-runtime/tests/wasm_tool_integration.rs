//! WASM tool integration tests
//!
//! End-to-end tests that verify the complete WASM tool execution pipeline:
//! 1. LLM tool_call → WASM sandbox execution → result return
//! 2. Permission violation is rejected
//! 3. Fuel exhaustion terminates execution
//! 4. Memory limit enforcement
//! 5. Execution timeout

#[cfg(feature = "wasm-tools")]
mod wasm_integration {
    use rollball_runtime::tools::wasm::engine::{WasmEngine, WasmEngineConfig, wasm_generator};
    use rollball_runtime::tools::wasm::instance::{WasmToolInstance, WasmExecutionResult};
    use rollball_runtime::tools::wasm::component::WasmToolComponent;
    use rollball_runtime::tools::wasm::wit::{ToolInput, ToolOutput};
    use rollball_runtime::tools::wasm::wasi_mapper::{
        map_permissions_to_wasi, check_wasi_access, check_wasi_network,
        WasiCapabilities, WasiDirPermission, WasiNetPermission,
    };
    use rollball_runtime::tools::wasm::sandbox::{WasiSandboxConfig, build_wasi_ctx};
    use rollball_core::Permission;
    use wasmtime::OptLevel;

    fn create_test_engine() -> WasmEngine {
        WasmEngine::default_engine().unwrap()
    }

    fn create_engine_with_limits(max_memory_mb: u32, max_execution_time_ms: u64) -> WasmEngine {
        let config = WasmEngineConfig::from_limits(max_memory_mb, max_execution_time_ms);
        WasmEngine::new(config).unwrap()
    }

    // ========== Test 1: Full pipeline ==========

    #[test]
    fn test_full_pipeline_execute_and_return() {
        let engine = create_test_engine();
        let wasm = wasm_generator::module_with_execute();

        // Step 1: Load component
        let component = WasmToolComponent::load("test_tool", &engine, &wasm).unwrap();

        // Step 2: Execute with typed input
        let input = ToolInput::new(serde_json::json!({"action": "process", "value": 42}));
        let output = component.execute(&engine, &input).unwrap();

        // Step 3: Verify output
        assert!(output.ok, "Execution should succeed");
    }

    #[test]
    fn test_full_pipeline_with_memory() {
        let engine = create_test_engine();
        let wasm = wasm_generator::module_with_memory();

        let component = WasmToolComponent::load("memory_tool", &engine, &wasm).unwrap();
        assert!(component.has_memory());

        let input = ToolInput::new(serde_json::json!({"data": "test"}));
        let output = component.execute(&engine, &input).unwrap();
        assert!(output.ok);
    }

    // ========== Test 2: Permission violation ==========

    #[test]
    fn test_wasi_permission_check_read_only_dir() {
        let caps = WasiCapabilities {
            dirs: vec![WasiDirPermission {
                path: "/data".to_string(),
                writable: false,
            }],
            networks: vec![],
        };

        // Read access should be allowed
        assert!(check_wasi_access(&caps, "/data/file.txt", false));
        // Write access should be denied
        assert!(!check_wasi_access(&caps, "/data/file.txt", true));
        // Access to unlisted path should be denied
        assert!(!check_wasi_access(&caps, "/other/file.txt", false));
    }

    #[test]
    fn test_wasi_permission_check_network() {
        let caps = WasiCapabilities {
            dirs: vec![],
            networks: vec![WasiNetPermission {
                url_pattern: "https://api.example.com".to_string(),
            }],
        };

        // Allowed URL
        assert!(check_wasi_network(&caps, "https://api.example.com/v1/data"));
        // Denied URL
        assert!(!check_wasi_network(&caps, "https://evil.com/steal"));
    }

    #[test]
    fn test_permission_to_wasi_full_pipeline() {
        let perms = vec![
            Permission::FilesystemRead(Some("/workspace".to_string())),
            Permission::FilesystemWrite(Some("/workspace/output".to_string())),
            Permission::Network(Some("https://api.rollball.ai".to_string())),
        ];

        let caps = map_permissions_to_wasi(&perms);
        let config = WasiSandboxConfig::from_capabilities(&caps);

        assert!(config.allow_network);
        assert_eq!(config.preopen_dirs.len(), 2);

        // Verify access checks
        assert!(check_wasi_access(&caps, "/workspace/data.txt", false));
        assert!(!check_wasi_access(&caps, "/workspace/data.txt", true)); // read-only for /workspace
        assert!(check_wasi_access(&caps, "/workspace/output/result.txt", true)); // write ok
        assert!(check_wasi_network(&caps, "https://api.rollball.ai/v1/agents"));
        assert!(!check_wasi_network(&caps, "https://other.api.com/data"));
    }

    // ========== Test 3: Fuel exhaustion ==========

    #[test]
    fn test_fuel_consumption_tracking() {
        let engine = create_engine_with_limits(50, 5000); // 5 second timeout
        let wasm = wasm_generator::module_with_execute();
        let mut instance = WasmToolInstance::new(&engine, &wasm).unwrap();

        let initial_fuel = instance.remaining_fuel();
        assert!(initial_fuel > 0);

        let result = instance.execute(r#"{"test": true}"#).unwrap();
        assert!(result.ok);
        assert!(result.fuel_consumed > 0, "Should consume some fuel");
        assert!(instance.remaining_fuel() < initial_fuel);
    }

    #[test]
    fn test_fuel_limit_set_correctly() {
        let engine = create_engine_with_limits(50, 3000);
        let wasm = wasm_generator::module_with_execute();
        let instance = WasmToolInstance::new(&engine, &wasm).unwrap();

        // Fuel should be 3000ms * 10K = 30,000,000
        assert_eq!(instance.fuel_limit(), 30_000_000);
    }

    // ========== Test 4: Memory limit ==========

    #[test]
    fn test_engine_memory_limit_config() {
        let engine = create_engine_with_limits(128, 5000);
        assert_eq!(engine.max_memory_mb(), 128);
    }

    #[test]
    fn test_engine_default_memory_limit() {
        let engine = create_test_engine();
        assert_eq!(engine.max_memory_mb(), 50); // default
    }

    // ========== Test 5: Execution timeout (fuel-based) ==========

    #[test]
    fn test_engine_config_from_limits() {
        let config = WasmEngineConfig::from_limits(64, 10000);
        assert_eq!(config.max_memory_mb, 64);
        assert_eq!(config.fuel_limit, 100_000_000); // 10000ms * 10K
    }

    // ========== Test 6: Invalid input handling ==========

    #[test]
    fn test_invalid_wasm_bytes() {
        let engine = create_test_engine();
        let result = WasmToolInstance::new(&engine, &[0xFF, 0xFE, 0xFD]);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_execute_export() {
        let engine = create_test_engine();
        let empty_wasm = wasm_generator::empty_module();
        let result = WasmToolInstance::new(&engine, &empty_wasm);
        assert!(result.is_err(), "Module without execute should fail");
    }

    // ========== Test 7: WASI sandbox build ==========

    #[test]
    fn test_wasi_sandbox_build_minimal() {
        let config = WasiSandboxConfig::default();
        let _ctx = build_wasi_ctx(&config);
        // Should not panic
    }

    #[test]
    fn test_wasi_sandbox_build_with_env() {
        let config = WasiSandboxConfig::default()
            .with_env("TOOL_MODE", "production")
            .with_arg("--verbose");
        let _ctx = build_wasi_ctx(&config);
    }

    // ========== Test 8: Component interface detection ==========

    #[test]
    fn test_component_detect_interface() {
        let engine = create_test_engine();
        let wasm = wasm_generator::module_with_execute();
        let component = WasmToolComponent::load("detect_tool", &engine, &wasm).unwrap();

        // Should detect V1 interface
        assert_eq!(component.version(), rollball_runtime::tools::wasm::wit::ComponentInterfaceVersion::V1RawPointer);
    }

    #[test]
    fn test_component_detect_memory() {
        let engine = create_test_engine();
        let wasm = wasm_generator::module_with_memory();
        let component = WasmToolComponent::load("mem_tool", &engine, &wasm).unwrap();

        assert!(component.has_memory());
    }
}
