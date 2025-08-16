use anyhow::Result;
use std::fs;
use std::fs::OpenOptions;
use std::process::Command;
use tempfile::NamedTempFile;
// crossterm は使用しない（WSL環境で問題が発生するため）
use crate::content::{ChapterData, ContentLoader, ExerciseData, StepData};
use std::io::{self, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;

pub struct VimTutorialGame {
    content_loader: ContentLoader,
    current_chapter: Option<ChapterData>,
    current_exercise_index: usize,
    current_step_index: usize,
}

// デバッグログ用のマクロ
macro_rules! debug_log {
    ($($arg:tt)*) => {
        let log_message = format!("[{}] 🔧 DEBUG: {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            format!($($arg)*));

        // ファイルにのみログを出力（コンソール出力は削除）
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/vim_tutorial_debug.log") {
            let _ = writeln!(file, "{}", log_message);
        }
    };
}

impl VimTutorialGame {
    pub fn new() -> Result<Self> {
        let content_loader = ContentLoader::new()?;

        // ログファイルを初期化
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("/tmp/vim_tutorial_debug.log")
        {
            let _ = writeln!(file, "=== Vim Tutorial Debug Log ===");
            let _ = writeln!(
                file,
                "Started at: {}",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
            );
        }

        debug_log!("ゲーム初期化完了");

        Ok(Self {
            content_loader,
            current_chapter: None,
            current_exercise_index: 0,
            current_step_index: 0,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        println!("=== Vim Tutorial Game (Neovim版) ===\n");
        println!("📄 デバッグログ: /tmp/vim_tutorial_debug.log");
        debug_log!("ゲーム開始");

        // 章選択メニューを表示
        self.show_chapter_menu()?;

        debug_log!("ゲーム終了");
        Ok(())
    }

    fn show_chapter_menu(&mut self) -> Result<()> {
        loop {
            println!("\n📚 === 章選択メニュー ===");
            self.content_loader.list_chapters();

            println!(
                "章番号を選択してください (1-{}, q=終了):",
                self.content_loader.get_chapter_count()
            );
            print!("選択: ");
            io::stdout().flush()?;

            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(0) => {
                    // EOF - デモモードで第1章を実行
                    println!("デモモード: 第1章を自動選択します。");
                    return self.start_chapter(1);
                }
                Ok(_) => {
                    let input = input.trim();

                    if input == "q" || input == "quit" {
                        println!("ゲームを終了します。");
                        break;
                    }

                    if let Ok(chapter_num) = input.parse::<u8>() {
                        if chapter_num >= 1
                            && chapter_num <= self.content_loader.get_chapter_count() as u8
                        {
                            return self.start_chapter(chapter_num);
                        } else {
                            println!(
                                "❌ 無効な章番号です。1-{} の範囲で入力してください。",
                                self.content_loader.get_chapter_count()
                            );
                        }
                    } else {
                        println!("❌ 数字または 'q' を入力してください。");
                    }
                }
                Err(_) => {
                    println!("デモモード: 第1章を自動選択します。");
                    return self.start_chapter(1);
                }
            }
        }

        Ok(())
    }

    fn start_chapter(&mut self, chapter_num: u8) -> Result<()> {
        if let Some(chapter) = self.content_loader.get_chapter(chapter_num) {
            self.current_chapter = Some(chapter.clone());
            self.current_exercise_index = 0;
            self.current_step_index = 0;

            println!(
                "\n🎯 === 第{}章: {} ===",
                chapter.chapter.number, chapter.chapter.title
            );
            println!("{}", chapter.chapter.description);
            println!();

            self.game_loop()?;
        } else {
            println!("❌ 第{}章が見つかりません。", chapter_num);
        }

        Ok(())
    }

    fn game_loop(&mut self) -> Result<()> {
        while let Some(chapter) = &self.current_chapter.clone() {
            if self.current_exercise_index >= chapter.exercises.len() {
                println!(
                    "🎉 第{}章「{}」を完了しました！",
                    chapter.chapter.number, chapter.chapter.title
                );
                println!("\nお疲れ様でした！");
                println!("\n他の章も学習してみましょう！");

                // 章の状態をリセット
                self.current_chapter = None;
                self.current_exercise_index = 0;
                self.current_step_index = 0;

                // メニューに戻る
                return self.show_chapter_menu();
            }

            let exercise = &chapter.exercises[self.current_exercise_index];

            if self.current_step_index >= exercise.steps.len() {
                println!("✅ 練習「{}」を完了しました！", exercise.title);
                self.current_exercise_index += 1;
                self.current_step_index = 0;
                println!("\n次の練習に進みます...\n");
                continue;
            }

            let step = &exercise.steps[self.current_step_index];

            // 練習情報を表示
            println!(
                "📝 === 練習 {}/{}: {} ===",
                self.current_exercise_index + 1,
                chapter.exercises.len(),
                exercise.title
            );
            println!("{}", exercise.description);
            println!();

            // サンプルコードを表示
            println!("サンプルコード:");
            for (i, line) in exercise.sample_code.iter().enumerate() {
                println!("{:2}: {}", i + 1, line);
            }
            println!();

            // ステップ情報を表示
            println!(
                "📍 ステップ {}/{}: {}",
                self.current_step_index + 1,
                exercise.steps.len(),
                step.instruction
            );
            println!("💡 解説: {}", step.explanation);
            println!("🎯 期待されるキー入力: {}", step.expected_input);

            // カーソル位置情報を表示
            if let Some(cursor_start) = step.cursor_start {
                println!(
                    "📌 開始位置: {}行{}列",
                    cursor_start[0] + 1,
                    cursor_start[1] + 1
                );
            }
            if let Some(cursor_end) = step.cursor_end {
                println!(
                    "🎯 目標位置: {}行{}列",
                    cursor_end[0] + 1,
                    cursor_end[1] + 1
                );
            }
            println!();
            println!("🚀 tmux分割画面でNeovimを起動します...");
            println!("上下の画面が表示されます。下の画面で実際にVim操作を練習してください！");
            println!();

            // 直接インタラクティブモードで実行
            if self.run_interactive_neovim(step)? {
                self.current_step_index += 1;
                println!("\n--- 次のステップ ---\n");
            }
        }

        Ok(())
    }

    fn run_interactive_neovim(&self, step: &StepData) -> Result<bool> {
        if let Some(chapter) = &self.current_chapter {
            let exercise = &chapter.exercises[self.current_exercise_index];

            // tmuxが利用可能かチェック
            if Command::new("tmux").arg("-V").output().is_ok() {
                println!("\n=== 🖥️ tmux分割画面モードで学習 ===");
                println!("指示とNeovim操作を同時に確認できます");
                return self.run_split_screen_neovim(exercise, step);
            } else {
                println!("❌ tmuxが利用できません。インストールしてください:");
                println!("sudo apt install tmux  または  brew install tmux");
                return Ok(false);
            }
        }

        Ok(false)
    }

    // 不要なメソッドを削除（tmuxのみ使用）

    fn run_split_screen_neovim(&self, exercise: &ExerciseData, step: &StepData) -> Result<bool> {
        println!("\n=== 🖥️  分割画面モードで練習 ===");
        debug_log!("分割画面モード開始");

        // サンプルファイルを作成
        let sample_content = exercise.sample_code.join("\n");
        let sample_file = NamedTempFile::new()?;
        fs::write(&sample_file, sample_content)?;
        debug_log!("サンプルファイル作成: {}", sample_file.path().display());

        // 状態監視用ファイル
        let status_file = "/tmp/vim_tutorial_status.json";
        debug_log!("状態監視ファイル: {}", status_file);

        // カーソル開始位置を決定
        let (start_row, start_col) = if let Some(cursor_start) = step.cursor_start {
            // YAMLは0ベース、Vimは1ベース
            (cursor_start[0] + 1, cursor_start[1] + 1)
        } else {
            (1, 1) // デフォルトは1行目1列目
        };

        // Neovim設定スクリプトを作成（状態監視付き）
        let nvim_script = format!(
            r#"
" 自動的にカーソル位置を監視（シンプル形式）
function! UpdateStatus()
  let line_num = line('.')
  let col_num = col('.')
  let mode_str = mode()
  let status_line = 'LINE:' . line_num . ',COL:' . col_num . ',MODE:' . mode_str
  call writefile([status_line], '{}')
endfunction

" カーソル移動時に状態更新
autocmd CursorMoved,CursorMovedI * call UpdateStatus()
autocmd InsertEnter,InsertLeave * call UpdateStatus()

" 初期状態を記録
call UpdateStatus()

" 指定された開始位置に移動（{}行{}列）
call cursor({}, {})

" 起動完了メッセージ
echo '🎯 学習開始！目標キー: {} | 開始位置: {}行{}列'
"#,
            status_file,
            start_row,
            start_col,
            start_row,
            start_col,
            step.expected_input,
            start_row,
            start_col
        );

        let script_file = NamedTempFile::new()?;
        fs::write(&script_file, nvim_script)?;
        debug_log!("Vimスクリプト作成: {}", script_file.path().display());

        // tmuxセッションを作成して画面分割
        let session_name = "vim_tutorial";
        debug_log!("tmuxセッション名: {}", session_name);

        // 既存セッションがあれば削除
        debug_log!("既存セッション削除中...");
        let kill_result = Command::new("tmux")
            .args(["kill-session", "-t", session_name])
            .output()?;
        debug_log!(
            "セッション削除結果: success={}, stderr={}",
            kill_result.status.success(),
            String::from_utf8_lossy(&kill_result.stderr)
        );

        // 新しいセッションを作成（上側ペイン：指示表示）
        debug_log!("新しいセッション作成中...");
        let new_session_result = Command::new("tmux")
            .args(["new-session", "-d", "-s", session_name])
            .output()?;
        debug_log!(
            "セッション作成結果: success={}, stderr={}",
            new_session_result.status.success(),
            String::from_utf8_lossy(&new_session_result.stderr)
        );

        if !new_session_result.status.success() {
            debug_log!("セッション作成失敗により処理中断");
            return Err(anyhow::anyhow!(
                "tmuxセッション作成に失敗: {}",
                String::from_utf8_lossy(&new_session_result.stderr)
            ));
        }

        // 画面を水平分割（下側ペイン：Neovim）
        debug_log!("画面分割中...");
        let split_result = Command::new("tmux")
            .args(["split-window", "-v", "-t", session_name])
            .output()?;
        debug_log!(
            "分割結果: success={}, stderr={}",
            split_result.status.success(),
            String::from_utf8_lossy(&split_result.stderr)
        );

        if !split_result.status.success() {
            debug_log!("画面分割失敗により処理中断");
            return Err(anyhow::anyhow!(
                "tmux画面分割に失敗: {}",
                String::from_utf8_lossy(&split_result.stderr)
            ));
        }

        // ペイン一覧を取得してデバッグ
        debug_log!("ペイン一覧取得中...");
        let pane_list_result = Command::new("tmux")
            .args([
                "list-panes",
                "-t",
                session_name,
                "-F",
                "#{pane_index}:#{pane_id}",
            ])
            .output()?;
        let pane_list = String::from_utf8_lossy(&pane_list_result.stdout);
        debug_log!("ペイン一覧: {}", pane_list.trim());

        // ペインIDを解析して配列に格納
        let panes: Vec<String> = pane_list
            .trim()
            .lines()
            .map(|line| {
                // "0:%2" のような形式から "%2" を取得
                line.split(':').nth(1).unwrap_or("").to_string()
            })
            .collect();

        debug_log!(
            "解析されたペイン: 上={:?}, 下={:?}",
            panes.first(),
            panes.get(1)
        );

        let top_pane = panes
            .first()
            .ok_or_else(|| anyhow::anyhow!("上ペインが見つかりません"))?;
        let bottom_pane = panes
            .get(1)
            .ok_or_else(|| anyhow::anyhow!("下ペインが見つかりません"))?;

        // 上側ペインで指示を表示（成功メッセージ監視付き）
        thread::sleep(Duration::from_millis(200));

        // 成功フラグファイル
        let success_flag = "/tmp/vim_tutorial_success.flag";
        let _ = fs::remove_file(success_flag); // 既存のフラグを削除

        let instruction_command = format!(
            r#"bash -c "clear; echo '=== 🎯 学習目標 ==='; echo '📝 {}'; echo '💡 解説: {}'; echo '🎯 期待キー: {}'; echo ''; echo '=== 📊 カーソル位置監視 ==='; echo '目標位置: {}行{}列'; echo '下のNeovimで操作してください！完了したら :q で終了'; echo ''; echo '📍 現在の状態: 学習中...'; while true; do if [ -f {} ]; then clear; echo '=== 🎯 学習目標 ==='; echo '📝 {}'; echo '💡 解説: {}'; echo '🎯 期待キー: {}'; echo ''; echo '=== 🎉 成功！ ==='; echo '✨ 目標達成しました！{}行{}列に到達！'; echo '素晴らしい！次のステップに進みましょう。'; echo '下のNeovimで :q を入力して終了してください。'; rm {}; sleep 2; break; else sleep 0.2; fi; done""#,
            step.instruction.replace("'", "'\"'\"'"),
            step.explanation.replace("'", "'\"'\"'"),
            step.expected_input.replace("'", "'\"'\"'"),
            step.cursor_end.map(|c| c[0] + 1).unwrap_or(1),
            step.cursor_end.map(|c| c[1] + 1).unwrap_or(1),
            success_flag,
            step.instruction.replace("'", "'\"'\"'"),
            step.explanation.replace("'", "'\"'\"'"),
            step.expected_input.replace("'", "'\"'\"'"),
            step.cursor_end.map(|c| c[0] + 1).unwrap_or(1),
            step.cursor_end.map(|c| c[1] + 1).unwrap_or(1),
            success_flag
        );

        debug_log!("上ペイン({})に指示送信中...", top_pane);
        debug_log!("指示コマンド: {}", instruction_command);
        let instruction_result = Command::new("tmux")
            .args(["send-keys", "-t", top_pane, &instruction_command, "Enter"])
            .output()?;
        debug_log!(
            "指示送信結果: success={}, stderr={}",
            instruction_result.status.success(),
            String::from_utf8_lossy(&instruction_result.stderr)
        );

        // 下側ペインでNeovimを起動（終了時にtmuxも終了するように）
        let nvim_command = format!(
            "nvim -S {} {}; tmux detach-client",
            script_file.path().display(),
            sample_file.path().display()
        );
        debug_log!("Neovimコマンド: {}", nvim_command);

        // 少し待ってからペインにコマンド送信
        thread::sleep(Duration::from_millis(100));

        debug_log!("下ペイン({})にNeovim起動コマンド送信中...", bottom_pane);
        let nvim_result = Command::new("tmux")
            .args(["send-keys", "-t", bottom_pane, &nvim_command, "Enter"])
            .output()?;
        debug_log!(
            "Neovim起動結果: success={}, stderr={}",
            nvim_result.status.success(),
            String::from_utf8_lossy(&nvim_result.stderr)
        );

        // Neovim起動失敗チェック
        if !nvim_result.status.success() {
            debug_log!("Neovim起動失敗により処理中断");
            return Err(anyhow::anyhow!(
                "Neovim起動に失敗: {}",
                String::from_utf8_lossy(&nvim_result.stderr)
            ));
        }

        debug_log!("Neovim起動成功、アタッチ準備中...");

        // Neovim起動直後のペイン状態を確認
        debug_log!("Neovim起動後のペイン確認...");
        let after_nvim = Command::new("tmux")
            .args(["list-panes", "-t", session_name])
            .output()?;
        debug_log!(
            "Neovim起動後ペイン: {}",
            String::from_utf8_lossy(&after_nvim.stdout)
        );

        // 少し待ってからNeovimペインにフォーカスを移動
        thread::sleep(Duration::from_millis(200));
        debug_log!("下ペインにフォーカス設定...");
        let _ = Command::new("tmux")
            .args(["select-pane", "-t", bottom_pane])
            .output();

        // フォーカス設定後のペイン状態を確認
        debug_log!("フォーカス設定後のペイン確認...");
        let after_focus = Command::new("tmux")
            .args(["list-panes", "-t", session_name])
            .output()?;
        debug_log!(
            "フォーカス後ペイン: {}",
            String::from_utf8_lossy(&after_focus.stdout)
        );

        // tmuxセッションにアタッチ
        println!("🚀 分割画面でNeovimを起動します...");
        println!("上部：指示とリアルタイム監視");
        println!("下部：Neovim操作画面");
        println!("終了：下部のNeovimで :q");

        // バックグラウンドで状態監視を開始
        debug_log!("状態監視スレッド開始");
        let status_file_copy = status_file.to_string();
        let step_copy = step.clone();
        let top_pane_copy = top_pane.clone();
        thread::spawn(move || {
            debug_log!("監視スレッド内開始");
            Self::monitor_neovim_status(&status_file_copy, step_copy, &top_pane_copy);
        });

        // tmuxにアタッチ前の最終チェック
        debug_log!("アタッチ前のセッション確認...");
        let list_result = Command::new("tmux").args(["list-sessions"]).output();
        debug_log!("現在のセッション一覧: {:?}", list_result);

        // レイアウト調整の前にペイン状態を確認
        thread::sleep(Duration::from_millis(100));

        debug_log!("レイアウト調整前のペイン確認...");
        let before_layout = Command::new("tmux")
            .args(["list-panes", "-t", session_name])
            .output()?;
        debug_log!(
            "レイアウト前ペイン: {}",
            String::from_utf8_lossy(&before_layout.stdout)
        );

        // 下ペインを選択してNeovimが操作できるようにする
        debug_log!("下ペインを選択...");
        let _ = Command::new("tmux")
            .args(["select-pane", "-t", bottom_pane])
            .output();

        // アタッチ前の最終的なペイン状態を確認
        debug_log!("最終ペイン状態確認中...");
        let final_panes = Command::new("tmux")
            .args([
                "list-panes",
                "-t",
                session_name,
                "-F",
                "#{pane_index}:#{pane_id}:#{pane_active}:#{pane_height}",
            ])
            .output()?;
        debug_log!(
            "最終ペイン状態: {}",
            String::from_utf8_lossy(&final_panes.stdout)
        );

        // ウィンドウの状態も確認
        let window_info = Command::new("tmux")
            .args([
                "list-windows",
                "-t",
                session_name,
                "-F",
                "#{window_index}:#{window_layout}",
            ])
            .output()?;
        debug_log!(
            "ウィンドウ状態: {}",
            String::from_utf8_lossy(&window_info.stdout)
        );

        // tmuxにアタッチ（分割表示を維持）
        debug_log!("tmuxセッションにアタッチ中...");
        println!("📱 tmuxセッションに接続中... (Ctrl+Dまたは:detachで終了)");
        println!("💡 上下の画面が表示されます。");
        println!("📌 操作方法:");
        println!("  - Ctrl+b ↑/↓: ペイン間の移動");
        println!("  - 下のペインでVim操作を行ってください");
        println!("  - 完了したら :q でVimを終了");

        // アタッチする前に画面をリフレッシュ
        let _ = Command::new("tmux")
            .args(["refresh-client", "-t", session_name])
            .output();

        let status = Command::new("tmux")
            .args(["attach-session", "-t", session_name])
            .status();

        match status {
            Ok(exit_status) => {
                debug_log!("tmuxセッション正常終了: success={}", exit_status.success());
            }
            Err(e) => {
                debug_log!("tmuxアタッチエラー: {}", e);
                return Err(anyhow::anyhow!("tmuxアタッチに失敗: {}", e));
            }
        }

        // セッション終了後のクリーンアップ
        debug_log!("クリーンアップ開始");

        // tmuxセッションを確実に削除
        debug_log!("tmuxセッション削除中...");
        let cleanup_result = Command::new("tmux")
            .args(["kill-session", "-t", session_name])
            .output();
        debug_log!("セッション削除結果: {:?}", cleanup_result);

        // 状態ファイルを削除
        let _ = fs::remove_file(status_file);
        let _ = fs::remove_file("/tmp/vim_tutorial_success.flag");
        debug_log!("状態ファイル削除完了");

        // ターミナルをクリア（元の画面に戻す）
        print!("\x1b[2J\x1b[H"); // 画面クリア + カーソルを左上に移動
        io::stdout().flush().unwrap_or(());

        println!("=== 練習完了 ===");
        println!("🎉 お疲れ様でした！分割画面での学習はいかがでしたか？");
        debug_log!("分割画面モード終了");
        Ok(true)
    }

    // 不要なメソッドを削除（tmuxのみ使用）

    fn monitor_neovim_status(status_file: &str, step: StepData, _top_pane: &str) {
        // 最初のログのみ出力
        debug_log!(
            "状態監視開始 - 目標: {}行{}列",
            step.cursor_end.map(|c| c[0] + 1).unwrap_or(1),
            step.cursor_end.map(|c| c[1] + 1).unwrap_or(1)
        );

        let mut last_position = (1, 1);
        let mut success_triggered = false;
        let target_position = if let Some(cursor_end) = step.cursor_end {
            (cursor_end[0] as i32 + 1, cursor_end[1] as i32 + 1)
        } else {
            return; // 目標位置が設定されていない場合は監視しない
        };

        loop {
            if let Ok(content) = fs::read_to_string(status_file) {
                // シンプルな形式で解析: "LINE:1,COL:2,MODE:n"
                for line in content.lines() {
                    if line.starts_with("LINE:") {
                        let parts: Vec<&str> = line.split(',').collect();
                        if parts.len() >= 2
                            && let (Ok(line_num), Ok(col_num)) = (
                                parts[0].strip_prefix("LINE:").unwrap_or("1").parse::<i32>(),
                                parts[1].strip_prefix("COL:").unwrap_or("1").parse::<i32>(),
                            )
                        {
                            let current_position = (line_num, col_num);

                            if current_position != last_position {
                                // 位置変更時のみログ出力
                                debug_log!("カーソル移動: {}行{}列", line_num, col_num);

                                // 目標達成時の処理
                                if current_position == target_position && !success_triggered {
                                    debug_log!(
                                        "🎉 目標達成！カーソル位置: {}行{}列",
                                        line_num,
                                        col_num
                                    );

                                    // 成功フラグファイルを作成
                                    let success_flag = "/tmp/vim_tutorial_success.flag";
                                    if let Ok(mut file) = OpenOptions::new()
                                        .create(true)
                                        .write(true)
                                        .truncate(true)
                                        .open(success_flag)
                                    {
                                        let _ = writeln!(file, "SUCCESS");
                                        debug_log!("成功フラグファイル作成: {}", success_flag);
                                    }
                                    success_triggered = true;
                                } else if current_position != target_position {
                                    debug_log!(
                                        "カーソル位置: {}行{}列 (目標: {}行{}列)",
                                        line_num,
                                        col_num,
                                        target_position.0,
                                        target_position.1
                                    );
                                }

                                last_position = current_position;
                            }
                            break;
                        }
                    }
                }
            }

            thread::sleep(Duration::from_millis(200));

            // ファイルが存在しなくなったら監視終了
            if !Path::new(status_file).exists() {
                debug_log!("状態監視終了");
                break;
            }
        }
    }
}
