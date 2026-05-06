use eva_runtime_with_task_validator::{
    handle_http_request, RuntimeCycleHttpResponse, RuntimeDaemonConfig,
};

#[test]
fn runtime_cycle_works_with_builtin_backend_without_network() {
    let config = RuntimeDaemonConfig::default();
    let response = handle_http_request(
        "POST",
        "/runtime/cycle",
        r#"{"goal":"prove offline daemon wiring","context":"builtin local model"}"#,
        &config,
    );

    assert_eq!(response.status_code, 200);
    let payload: RuntimeCycleHttpResponse =
        serde_json::from_str(&response.body).expect("runtime response JSON");
    assert_eq!(payload.model_advisory.status, "ok");
    assert_eq!(payload.model_advisory.model_id, "eva-lite");
    assert!(payload
        .model_advisory
        .content
        .as_deref()
        .unwrap_or_default()
        .contains("eva-lite advisory"));
    assert_eq!(payload.runtime_audit.learning_bias_applied, true);
}
