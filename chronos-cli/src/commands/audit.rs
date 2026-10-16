//! `chronos audit <file>` — Full mutation gate + blast-radius analysis
//! `chronos audit --diff` — Git diff targeted blast-radius

use anyhow::Result;
use chronos_core::{
    ast::dispatcher::LanguageDispatcher,
    blast_radius::{
        impact_matrix::ImpactAnalyzer,
        markdown_glance::generate_glance_markdown,
        visualizer::generate_html_dashboard,
    },
    graph::schema::{ChronosGraph, NodeKind},
    mutation::arena::{MutationArena, MUTATION_SCORE_THRESHOLD, MUTATION_SCORE_THRESHOLD_STRICT},
    mutation::watchdog::WatchdogConfig,
};
use std::path::PathBuf;

pub async fn run(
    workspace: PathBuf,
    file: PathBuf,
    strict: bool,
    test_cmd: String,
) -> Result<()> {
    let abs_file = if file.is_absolute() { file.clone() } else { workspace.join(&file) };

    println!("🔬 CHRONOS AUDIT — {}", abs_file.display());
    println!("{}", "─".repeat(60));

    // ── Step 1: AST Parse ──────────────────────────────────────────
    println!("\n📖 Step 1/3 — Parsing AST...");
    let mut dispatcher = LanguageDispatcher::new()?;
    let parsed = dispatcher.parse_file(&abs_file)?;

    let mut graph = ChronosGraph::new();
    let (_inserted, edges_added) = graph.ingest_parsed_module(&parsed);

    let report = graph.status_report();
    println!(
        "   ✓ {} nodes (functions: {}, tests: {}, routes: {}, edges: {})",
        report.total_nodes,
        report.function_nodes,
        report.test_nodes,
        graph.nodes_by_kind(&NodeKind::ApiRoute).len(),
        edges_added,
    );

    // ── Step 2: Blast-Radius ──────────────────────────────────────
    println!("\n💥 Step 2/3 — Computing blast-radius...");
    let analyzer = ImpactAnalyzer::new(&graph);

    // Analyze all function/class nodes in the file
    let fn_nodes = graph.nodes_by_kind(&NodeKind::Function);
    let class_nodes = graph.nodes_by_kind(&NodeKind::Class);
    let all_sym_nodes: Vec<_> = fn_nodes.iter().chain(class_nodes.iter()).collect();

    if all_sym_nodes.is_empty() {
        println!("   ℹ️  No function/class symbols found — skipping blast-radius.");
    } else {
        // Find highest risk symbol
        let _reports: Vec<_> = all_sym_nodes.iter()
            .map(|n| (n, analyzer.analyze(&n.id, false)))
            .collect();

        let changed_ids: Vec<_> = all_sym_nodes.iter().map(|n| &n.id).collect();
        let diff_report = analyzer.analyze_diff(&changed_ids);
        let glance = generate_glance_markdown(&diff_report);
        println!("{}", glance);

        if diff_report.needs_visual_dashboard() {
            let html = generate_html_dashboard(&diff_report);
            let html_path = workspace.join(format!(
                "chronos_blast_radius_{}.html",
                abs_file.file_stem().unwrap_or_default().to_string_lossy()
            ));
            std::fs::write(&html_path, &html)?;
            println!("\n📊 HTML Dashboard: {}", html_path.display());
        }
    }

    // ── Step 3: Mutation Gate ─────────────────────────────────────
    let threshold = if strict { MUTATION_SCORE_THRESHOLD_STRICT } else { MUTATION_SCORE_THRESHOLD };
    println!("\n🧬 Step 3/3 — Mutation Gate ({:.0}% threshold)...", threshold * 100.0);

    let test_parts: Vec<String> = test_cmd.split_whitespace().map(|s| s.to_string()).collect();
    let (runner, args) = if test_parts.is_empty() {
        ("npx".to_string(), vec!["jest".to_string()])
    } else {
        (test_parts[0].clone(), test_parts[1..].to_vec())
    };

    let watchdog = WatchdogConfig {
        timeout_ms: 5000,
        max_concurrency: 4,
        test_runner: runner,
        test_args: args,
    };

    let arena = MutationArena::new(&graph).with_config(watchdog);
    let mutation_report = arena.run(&abs_file, None, &workspace).await?;

    println!("{}", mutation_report.format_for_agent());

    // ── Summary ───────────────────────────────────────────────────
    println!("\n{}", "═".repeat(60));
    let gate = if mutation_report.passed_quality_gate { "✅ PASS" } else { "❌ FAIL" };
    println!("CHRONOS AUDIT RESULT: {gate}");
    println!(
        "Mutation score: {:.1}% ({}/{} killed) | Gate: {:.0}%",
        mutation_report.mutation_score * 100.0,
        mutation_report.killed_mutants,
        mutation_report.total_mutants,
        threshold * 100.0,
    );

    if !mutation_report.passed_quality_gate {
        if let Some(prompt) = mutation_report.generate_ai_feedback_prompt() {
            println!("\n🤖 AI Feedback:\n{}", prompt);
        }
        std::process::exit(1); // Non-zero exit = CI/CD can catch this
    }

    Ok(())
}
