use maud::{html, Markup, PreEscaped, DOCTYPE};
use std::time::Duration;

use crate::FuzzStats;

// ===========================================================================
// Report generation
// ===========================================================================

pub fn generate_report(stats: &FuzzStats, target: &str, elapsed: Duration) -> Markup {
    let total_execs = stats.execs_per_dp.iter().max().copied().unwrap_or(0);
    let peak_exec_s = stats
        .timeline
        .iter()
        .map(|d| d.exec_per_sec)
        .fold(0.0_f64, f64::max);
    let avg_exec_s = if !stats.timeline.is_empty() {
        stats
            .timeline
            .iter()
            .map(|d| d.exec_per_sec)
            .sum::<f64>()
            / stats.timeline.len() as f64
    } else {
        0.0
    };

    let elapsed_secs = elapsed.as_secs_f64();
    let has_timeline = !stats.timeline.is_empty();
    let progress_pct = if elapsed_secs > 0.0 { 100.0_f64.min(100.0) } else { 0.0 };

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Fuzz Report — " (target) }
                script src="https://cdn.tailwindcss.com" {}
                script src="https://cdn.jsdelivr.net/npm/chart.js" {}
                style {
                    "body { font-family: 'Inter', system-ui, -apple-system, sans-serif; }"
                }
            }
            body class="bg-gray-50 min-h-screen p-6" {
                div class="max-w-6xl mx-auto" {

                    // ── Header ──
                    div class="mb-8" {
                        h1 class="text-3xl font-bold text-gray-900" { "Fuzzing Campaign Report" }
                        p class="text-gray-500 mt-1" {
                            "Target: " code class="bg-gray-200 rounded px-2 py-0.5 text-sm" { (target) }
                            "  •  Run duration: " strong { (fmt_duration(elapsed)) }
                        }
                    }

                    // ── Summary cards ──
                    div class="grid grid-cols-2 md:grid-cols-4 gap-4 mb-8" {
                        (summary_card("#ff6b6b", "Crashes", stats.crashes.len(), "Total unique crashes"))
                        (summary_card("#51cf66", "Corpus", stats.corpus_entries, &format!("{} files / {} kb", stats.corpus_entries, stats.corpus_bytes / 1024)))
                        (summary_card("#339af0", "Executions", total_execs, "Total iterations"))
                        (summary_card("#fcc419", "Peak exec/s", peak_exec_s as u64, &format!("Avg: {:.0}/s", avg_exec_s)))
                    }

                    // ── Progress bar (time-based) ──
                    div class="bg-white rounded-xl shadow-sm p-6 mb-8" {
                        h2 class="text-lg font-semibold text-gray-800 mb-3" { "Campaign Progress" }
                        div class="w-full bg-gray-200 rounded-full h-4 overflow-hidden" {
                            div class="bg-gradient-to-r from-blue-500 to-blue-600 h-4 rounded-full transition-all duration-500"
                                style={ "width: " (progress_pct) "%;" } {}
                        }
                        p class="text-sm text-gray-500 mt-2" {
                            (if has_timeline { format!("{} data points collected", stats.timeline.len()) } else { "No live fuzzer output captured".to_string() })
                        }
                    }

                    // ── Performance chart ──
                    div class="bg-white rounded-xl shadow-sm p-6 mb-8" {
                        h2 class="text-lg font-semibold text-gray-800 mb-3" { "Execution Speed Over Time" }
                        canvas id="speedChart" height="200" {}
                    }

                    // ── Crash details ──
                    div class="bg-white rounded-xl shadow-sm p-6" {
                        h2 class="text-lg font-semibold text-gray-800 mb-3" {
                            "Crash Artifacts"
                            span class="ml-2 text-sm font-normal text-gray-400" { "(" (stats.crashes.len()) " found)" }
                        }
                        @if stats.crashes.is_empty() {
                            p class="text-gray-400 italic py-8 text-center" { "No crashes detected — the fuzzer ran clean." }
                        } @else {
                            div class="overflow-x-auto" {
                                table class="w-full text-left" {
                                    thead {
                                        tr class="border-b border-gray-200 text-sm text-gray-500 uppercase tracking-wide" {
                                            th class="py-3 px-4 font-medium" { "File" }
                                            th class="py-3 px-4 font-medium" { "Size" }
                                            th class="py-3 px-4 font-medium" { "Modified" }
                                        }
                                    }
                                    tbody {
                                        @for crash in &stats.crashes {
                                            tr class="border-b border-gray-100 hover:bg-gray-50 transition-colors" {
                                                td class="py-3 px-4 font-mono text-sm text-gray-700" { (crash.path) }
                                                td class="py-3 px-4 text-sm text-gray-600" { (fmt_size(crash.size)) }
                                                td class="py-3 px-4 text-sm text-gray-500" { (crash.modified) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Chart.js init ──
                script {
                    "const ctx = document.getElementById('speedChart').getContext('2d');"
                    "const data = " (PreEscaped(timeline_json(stats))) ";"
                    "new Chart(ctx, {"
                        "type: 'line',"
                        "data: {"
                            "labels: data.map((_, i) => i),"
                            "datasets: [{"
                                "label: 'exec/s',"
                                "data: data.map(d => d.exec_per_sec),"
                                "borderColor: '#339af0',"
                                "backgroundColor: 'rgba(51,154,240,0.1)',"
                                "fill: true,"
                                "tension: 0.3,"
                                "pointRadius: 1,"
                            "},{"
                                "label: 'Corpus',"
                                "data: data.map(d => d.corpus_size),"
                                "borderColor: '#51cf66',"
                                "backgroundColor: 'rgba(81,207,102,0.1)',"
                                "fill: true,"
                                "tension: 0.3,"
                                "pointRadius: 1,"
                                "yAxisID: 'y1',"
                            "}]"
                        "},"
                        "options: {"
                            "responsive: true,"
                            "interaction: { intersect: false, mode: 'index' },"
                            "scales: {"
                                "x: { title: { display: true, text: 'Sample' } },"
                                "y: { title: { display: true, text: 'exec/s' }, beginAtZero: true },"
                                "y1: { position: 'right', title: { display: true, text: 'Corpus' }, beginAtZero: true, grid: { drawOnChartArea: false } }"
                            "},"
                            "plugins: {"
                                "legend: { position: 'top' }"
                            "}"
                        "}"
                    "});"
                }
            }
        }
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

fn summary_card(color: &str, label: &str, value: impl std::fmt::Display, hint: &str) -> Markup {
    html! {
        div class="bg-white rounded-xl shadow-sm p-5 border-l-4" style={ "border-left-color: " (color) ";" } {
            p class="text-sm text-gray-500 uppercase tracking-wide" { (label) }
            p class="text-3xl font-bold mt-1" style={ "color: " (color) ";" } { (value) }
            p class="text-xs text-gray-400 mt-1 truncate" { (hint) }
        }
    }
}

fn fmt_duration(d: Duration) -> String {
    let total = d.as_secs_f64();
    if total < 60.0 {
        format!("{total:.1}s")
    } else if total < 3600.0 {
        format!("{:.0}m {:.0}s", total / 60.0, total % 60.0)
    } else {
        format!("{:.1}h", total / 3600.0)
    }
}

fn fmt_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn timeline_json(stats: &FuzzStats) -> String {
    serde_json::to_string(&stats.timeline).unwrap_or_else(|_| "[]".to_string())
}
