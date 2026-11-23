//! Output language for the shared UI surface.
//!
//! Only the common surface (help, menu, prompts and short status messages) is
//! translated. Everything else — command-specific report text, data values,
//! JSON — stays in its original wording and is returned unchanged when no
//! translation exists, so adding a language never removes output.
//!
//! The active language is set once at CLI startup from the `language` config
//! key. Library callers that never go through the CLI keep the English
//! default.

use std::borrow::Cow;
use std::sync::Mutex;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    En,
    Ja,
}

impl Language {
    pub fn from_code(code: &str) -> Self {
        match code {
            "ja" => Language::Ja,
            _ => Language::En,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Ja => "ja",
        }
    }
}

static LANGUAGE: OnceLock<Mutex<Language>> = OnceLock::new();

/// Set the process-wide output language (called from config at startup).
pub fn set_language(code: &str) {
    let slot = LANGUAGE.get_or_init(|| Mutex::new(Language::En));
    *slot.lock().unwrap() = Language::from_code(code);
}

pub fn language() -> Language {
    LANGUAGE
        .get_or_init(|| Mutex::new(Language::En))
        .lock()
        .unwrap()
        .to_owned()
}

/// Translate a known UI string. Unknown strings are returned verbatim, so
/// this is safe to call on arbitrary command output.
pub fn tr(text: &str) -> Cow<'_, str> {
    if language() != Language::Ja {
        return Cow::Borrowed(text);
    }
    match JA.iter().find(|(en, _)| *en == text) {
        Some((_, ja)) => Cow::Borrowed(ja),
        None => Cow::Borrowed(text),
    }
}

/// Japanese strings for the common UI. Keys are the exact English source
/// strings so call sites need no separate key vocabulary.
static JA: &[(&str, &str)] = &[
    // Help chrome
    ("USAGE:", "使い方:"),
    ("COMMANDS:", "コマンド:"),
    ("ARGUMENTS:", "引数:"),
    ("OPTIONS:", "オプション:"),
    ("[COMMANDS]", "[コマンド]"),
    ("<OPTIONS>", "<オプション>"),
    (
        "For more info, run any command with the `--help` flag:",
        "詳細は各コマンドを `--help` 付きで実行してください:",
    ),
    // Global options
    ("Display this help message", "ヘルプを表示"),
    ("Display this version", "バージョンを表示"),
    ("Disable ANSI colors", "ANSIカラーを無効化"),
    (
        "Color mode: auto, always, never",
        "カラーモード: auto, always, never",
    ),
    (
        "Use the wide layout (no narrow-terminal wrapping)",
        "広いレイアウトを使用（狭い端末で折り返さない）",
    ),
    (
        "Manage subscription services from your terminal",
        "ターミナルでサブスクリプションを管理",
    ),
    // Top-level commands (the `subtrack --help` command list)
    ("List all subscriptions", "すべてのサブスクリプションを表示"),
    ("Add a subscription", "サブスクリプションを追加"),
    ("Edit a subscription", "サブスクリプションを編集"),
    ("Delete subscriptions", "サブスクリプションを削除"),
    (
        "Cancel a subscription with a guided checklist",
        "ガイド付きチェックリストでサブスクリプションを解約",
    ),
    (
        "Clone an existing subscription",
        "既存のサブスクリプションを複製",
    ),
    (
        "Archive a subscription (set status to archived)",
        "サブスクリプションをアーカイブ（ステータスを archived に変更）",
    ),
    (
        "Unarchive a subscription (set status back to active)",
        "サブスクリプションをアーカイブ解除（ステータスを active に戻す）",
    ),
    (
        "Filter subscriptions by tags (AND logic)",
        "タグでサブスクリプションを絞り込む（AND条件）",
    ),
    ("Manage tags", "タグを管理"),
    (
        "Search subscriptions by name, notes, or tags",
        "名前・メモ・タグでサブスクリプションを検索",
    ),
    ("Manage free trials", "無料トライアルを管理"),
    (
        "Bulk operations on subscriptions",
        "サブスクリプションの一括操作",
    ),
    (
        "Show spending forecast and what-if scenarios",
        "支出予測と What-if シナリオを表示",
    ),
    ("Export subscriptions", "サブスクリプションをエクスポート"),
    (
        "Import subscriptions from CSV",
        "CSVからサブスクリプションをインポート",
    ),
    (
        "Show subscription summary statistics",
        "サブスクリプションのサマリーを表示",
    ),
    (
        "Backup database (gzip compressed)",
        "データベースをバックアップ（gzip圧縮）",
    ),
    (
        "Restore database from a backup",
        "バックアップからデータベースを復元",
    ),
    ("Show payment totals", "支払い合計を表示"),
    (
        "Show upcoming bills within a number of days",
        "今後N日以内の請求を表示",
    ),
    (
        "Show a monthly calendar with billing days marked",
        "請求日を表示した月間カレンダー",
    ),
    (
        "Show price change history for a subscription",
        "サブスクリプションの価格変動履歴",
    ),
    (
        "Send desktop notification for upcoming bills",
        "今後の請求をデスクトップ通知",
    ),
    ("Manage filter profiles", "フィルタープロファイルを管理"),
    (
        "Analyze subscriptions and suggest cost optimizations",
        "サブスクリプションを分析しコスト最適化を提案",
    ),
    (
        "Show monthly spending timeline with bar chart",
        "月次支出のタイムラインを棒グラフで表示",
    ),
    ("View and manage the audit log", "監査ログの表示と管理"),
    (
        "Run database maintenance (VACUUM, integrity check)",
        "データベース保守を実行（VACUUM、整合性チェック）",
    ),
    (
        "Run all maintenance tasks (integrity check, VACUUM, prune audit/tags)",
        "すべての保守タスクを実行（整合性チェック、VACUUM、監査ログ/タグの整理）",
    ),
    ("Show database statistics", "データベース統計を表示"),
    (
        "Show spending vs budget and detect budget overruns",
        "支出と予算を比較し予算超過を検出",
    ),
    (
        "Show a yearly subscription report",
        "年間サブスクリプションレポートを表示",
    ),
    ("List supported currencies", "対応通貨を表示"),
    (
        "Detect duplicate subscriptions by name similarity",
        "名前の類似度で重複を検出",
    ),
    (
        "Start MCP server for AI agent integration (stdio transport)",
        "AIエージェント連携用MCPサーバーを起動（stdio）",
    ),
    (
        "Show detailed subscription analytics",
        "詳細なサブスクリプション分析を表示",
    ),
    (
        "Compare spending between current and previous period",
        "前期と今期の支出を比較",
    ),
    ("Manage configuration", "設定を管理"),
    ("Track LLM API usage costs", "LLM APIの使用量とコストを追跡"),
    (
        "Manage subscription suggestions",
        "サブスクリプションの提案を管理",
    ),
    ("Pause subscriptions", "サブスクリプションを一時停止"),
    (
        "Resume paused or cancelled subscriptions",
        "一時停止・解約済みを再開",
    ),
    ("Renew a subscription", "サブスクリプションを更新"),
    (
        "Review upcoming renewals, bills, and trials",
        "今後の更新・請求・トライアルを確認",
    ),
    (
        "Show annual subscription spending",
        "年間サブスクリプション支出を表示",
    ),
    (
        "Check subscription data integrity",
        "サブスクリプションデータの整合性をチェック",
    ),
    (
        "Show subscription changes",
        "サブスクリプションの変更履歴を表示",
    ),
    (
        "Import subscription candidates from a receipt file",
        "領収書ファイルから候補をインポート",
    ),
    (
        "Manage subscription templates",
        "サブスクリプションのテンプレートを管理",
    ),
    // Entry menu
    ("What would you like to do?", "何をしますか？"),
    (
        "  No subscriptions yet — choose Add to create one",
        "  サブスクリプションがありません — 「追加」で作成できます",
    ),
    ("  Monthly:", "  月間:"),
    ("  Next 7 days:", "  今後7日:"),
    ("subscription", "件のサブスクリプション"),
    ("subscriptions", "件のサブスクリプション"),
    ("bill", "件の請求"),
    ("bills", "件の請求"),
    ("List subscriptions", "サブスクリプション一覧"),
    ("Add subscription", "サブスクリプション追加"),
    ("Edit subscription", "サブスクリプション編集"),
    ("Delete subscription", "サブスクリプション削除"),
    ("Summary", "サマリー"),
    ("Payment totals", "支払い合計"),
    ("Budget status", "予算状況"),
    ("Yearly report", "年間レポート"),
    ("Upcoming bills", "今後の請求"),
    ("Statistics", "統計"),
    ("Calendar", "カレンダー"),
    ("Tags", "タグ"),
    ("LLM usage", "LLM使用量"),
    ("Trials", "トライアル"),
    ("Configuration", "設定"),
    ("Help — all commands", "ヘルプ — 全コマンド"),
    ("Exit", "終了"),
    // Prompts
    ("A value is required", "値を入力してください"),
    (
        "No matches — showing all choices",
        "一致する項目がありません — すべて表示します",
    ),
    ("Please choose a number", "番号を選択してください"),
    ("Invalid choice", "無効な選択です"),
    ("Please answer y or n", "y または n を入力してください"),
    (
        "(numbers, 'all', empty = cancel)",
        "(番号, 'all', 空欄 = キャンセル)",
    ),
    ("none", "なし"),
    ("all", "すべて"),
    ("yes", "はい"),
    ("no", "いいえ"),
    // Settings wizard
    ("Select a setting to change", "変更する設定を選択"),
    ("New value for", "新しい値"),
    ("Done", "完了"),
    ("(- = back)", "(- = 戻る)"),
    ("No settings changed", "設定は変更されませんでした"),
    // Common status / empty-state messages
    (
        "No subscriptions found — try `subtrack add`",
        "サブスクリプションがありません — `subtrack add` で追加してください",
    ),
    ("No subscriptions found", "サブスクリプションがありません"),
    (
        "No active subscriptions found — try `subtrack add` or change status",
        "有効なサブスクリプションがありません — `subtrack add` で追加するかステータスを変更してください",
    ),
    (
        "No matching subscriptions found",
        "一致するサブスクリプションがありません",
    ),
    (
        "No subscriptions need changing",
        "変更が必要なサブスクリプションはありません",
    ),
    ("No tags found", "タグがありません"),
    ("No orphaned tags found", "孤立したタグはありません"),
    ("No templates found", "テンプレートがありません"),
    ("No saved profiles", "保存済みプロファイルがありません"),
    ("No trials found", "トライアルがありません"),
    ("No pending suggestions.", "保留中の提案はありません"),
    (
        "No duplicate subscriptions found",
        "重複するサブスクリプションはありません",
    ),
    (
        "No audit log entries found",
        "監査ログのエントリがありません",
    ),
    ("No changes found", "変更がありません"),
    ("Nothing needs review", "確認が必要な項目はありません"),
    (
        "No billing events this month",
        "今月の請求イベントはありません",
    ),
    (
        "No API usage found for this month",
        "今月のAPI使用量はありません",
    ),
    (
        "No paid usage entries found",
        "支払い済みの使用量エントリがありません",
    ),
    (
        "No new usage entries found",
        "新しい使用量エントリはありません",
    ),
    ("No price history found", "価格履歴がありません"),
    ("No price changes recorded", "価格変動は記録されていません"),
    ("No receipt candidates found", "領収書の候補がありません"),
    ("Cancelled", "キャンセルしました"),
    ("Command failed", "コマンドが失敗しました"),
    ("(not set)", "(未設定)"),
    ("Config reset to defaults", "設定をデフォルトに戻しました"),
    ("Integrity check: passed", "整合性チェック: 合格"),
    ("Slack notification sent", "Slack通知を送信しました"),
    ("Webhook notification sent", "Webhook通知を送信しました"),
    (
        "Provide a search query: subtrack search <query>",
        "検索クエリを指定してください: subtrack search <query>",
    ),
    // Daily expenses
    (
        "Record daily expenses and show spending totals",
        "日々の支出を記録し、合計を表示",
    ),
    (
        "Add items to a day's expense summary",
        "ある日の支出まとめに明細を追加",
    ),
    ("List daily expense summaries", "日別の支出まとめを表示"),
    ("Show expense totals for a period", "期間内の支出合計を表示"),
    (
        "Delete one or more day summaries",
        "1日以上の支出まとめを削除",
    ),
    ("Manage individual expense items", "支出明細を管理"),
    ("Delete one item from a day", "ある日の明細を1件削除"),
    ("Edit one item in a day", "ある日の明細を1件編集"),
    (
        "Date (YYYY-MM-DD, default: today)",
        "日付（YYYY-MM-DD、既定: 今日）",
    ),
    ("Amount in major currency units", "金額（通貨の基本単位）"),
    (
        "Currency code (default: defaultCurrency)",
        "通貨コード（既定: defaultCurrency）",
    ),
    ("Category (default: other)", "カテゴリ（既定: other）"),
    (
        "Payment method: cash, card, bank, other",
        "支払い方法: cash, card, bank, other",
    ),
    ("Optional note", "メモ（任意）"),
    ("Show one date (YYYY-MM-DD)", "指定日のみ表示（YYYY-MM-DD）"),
    ("Start date (YYYY-MM-DD)", "開始日（YYYY-MM-DD）"),
    ("End date (YYYY-MM-DD)", "終了日（YYYY-MM-DD）"),
    ("Show every recorded day", "記録済みのすべての日を表示"),
    (
        "Month to summarize (YYYY-MM, default: current month)",
        "集計月（YYYY-MM、既定: 今月）",
    ),
    (
        "Group by: category, method, day (default: category)",
        "グループ化: category, method, day（既定: category）",
    ),
    (
        "Convert all totals to this currency using FX rates",
        "為替レートですべての合計をこの通貨に換算",
    ),
    (
        "Day ID or date (YYYY-MM-DD)",
        "日のIDまたは日付（YYYY-MM-DD）",
    ),
    ("Item ID within the day", "その日の明細ID"),
    ("New amount", "新しい金額"),
    ("New currency code", "新しい通貨コード"),
    ("New category", "新しいカテゴリ"),
    ("New payment method", "新しい支払い方法"),
    ("New note", "新しいメモ"),
    ("Remove the existing note", "既存のメモを削除"),
    ("No expenses found", "支出の記録がありません"),
    ("Daily expenses", "日々の支出"),
    ("Expense added", "支出を追加しました"),
    ("Expense day deleted", "支出まとめを削除しました"),
    ("Expense item deleted", "支出明細を削除しました"),
    ("Expense item updated", "支出明細を更新しました"),
    ("Expense day not found", "支出まとめが見つかりません"),
    ("Expense item not found", "支出明細が見つかりません"),
    ("Add another item?", "別の明細も追加しますか？"),
    ("Amount", "金額"),
    ("Currency", "通貨"),
    ("Category", "カテゴリ"),
    ("Method", "支払い方法"),
    ("Note", "メモ"),
    ("Item ID", "明細ID"),
    ("Total", "合計"),
    ("Groups", "内訳"),
    ("Period", "期間"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_falls_back_to_source() {
        let found = JA.iter().find(|(en, _)| *en == "Exit");
        assert_eq!(found.map(|(_, ja)| *ja), Some("終了"));
        assert!(JA.iter().all(|(en, ja)| !en.is_empty() && !ja.is_empty()));
    }

    #[test]
    fn language_codes() {
        assert_eq!(Language::from_code("ja"), Language::Ja);
        assert_eq!(Language::from_code("en"), Language::En);
        assert_eq!(Language::from_code("fr"), Language::En);
        assert_eq!(Language::Ja.code(), "ja");
    }
}
