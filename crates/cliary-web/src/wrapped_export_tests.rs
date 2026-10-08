use super::*;
use cliary_core::{CountItem, NewToolInsight, WrappedInsights};
#[test]
fn offline_report_is_script_free_escaped_and_retains_facts() {
    let report = Wrapped {
        year: 2024,
        total_runs: 3,
        top_tools: vec![CountItem {
            name: "<script>alert(1)</script>".into(),
            count: 3,
        }],
        insights: WrappedInsights {
            new_tools_count: 1,
            breakout_tool: Some(NewToolInsight {
                name: "<img src=x onerror=alert(1)>".into(),
                runs: 3,
                first_recorded: "2024-01-01".into(),
            }),
            source_counts: vec![CountItem {
                name: "zsh".into(),
                count: 3,
            }],
            undated_tools: 2,
            ..Default::default()
        },
        ..Default::default()
    };
    for lang in ["en", "zh-CN"] {
        let body = standalone(&report, lang);
        assert!(!body.contains("<script"));
        assert!(!body.contains("<img"));
        assert!(!body.contains("<form"));
        assert!(!body.contains("<a "));
        assert!(body.contains("&lt;script&gt;"));
        assert!(body.contains("default-src 'none'"));
        assert!(body.contains("zsh"));
        assert!(body.contains("2024"));
        assert!(!body.contains("/assets/"));
    }
    let known = HashSet::from(["ncdu".into()]);
    assert!(tool_label("ncdu", &known, false).contains("/tools/ncdu"));
    assert!(!tool_label("retired", &known, false).contains("href"));
}
#[tokio::test]
async fn export_validates_inputs_and_sets_private_download_headers() {
    let root = tempfile::tempdir().unwrap();
    let app = App {
        core: std::sync::Arc::new(
            cliary_core::Cliary::at(cliary_core::Paths::new(
                root.path().join("config"),
                root.path().join("data"),
                root.path().join("cache"),
            ))
            .unwrap(),
        ),
        csrf: "test".into(),
    };
    let response = export(
        State(app.clone()),
        Query(ExportQuery {
            year: Some("2024".into()),
            format: Some("json".into()),
        }),
    )
    .await
    .unwrap();
    assert_eq!(response.headers()["content-type"], "application/json");
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(
        response.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("2024.json")
    );
    for (year, format) in [("0", "html"), ("2024", "javascript"), ("<img>", "json")] {
        let result = export(
            State(app.clone()),
            Query(ExportQuery {
                year: Some(year.into()),
                format: Some(format.into()),
            }),
        )
        .await;
        assert_eq!(result.err().unwrap().0, StatusCode::BAD_REQUEST);
    }
}
