use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::OpenOptions;
// use std::io::{self, Write};
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;
use tempfile::NamedTempFile;

use crate::vim_rpc::VimRpcClient;
use crate::vim_state::{Goal, GoalDetector, GoalType, VimMode, VimState};

// デバッグログ用のマクロ
macro_rules! debug_log {
    ($($arg:tt)*) => {
        let log_message = format!("[{}] 🔧 CONTINUOUS_DEBUG: {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            format!($($arg)*));

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/vim_continuous_debug.log") {
            let _ = writeln!(file, "{}", log_message);
        }
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousExercise {
    pub title: String,
    pub description: String,
    pub sample_code: Vec<String>,
    pub goals: Vec<ExerciseGoal>,
    pub flow_type: FlowType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExerciseGoal {
    #[serde(rename = "type")]
    pub goal_type: String,
    pub target: serde_json::Value,
    pub description: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlowType {
    #[serde(rename = "sequential")]
    Sequential, // 順番に実行する必要がある
    #[serde(rename = "any_order")]
    AnyOrder, // 順不同で実行可能
    #[serde(rename = "parallel")]
    Parallel, // 複数の目標を同時に達成
}

pub struct ContinuousVimSession {
    vim_client: VimRpcClient,
    goal_detector: GoalDetector,
    current_exercise: Option<ContinuousExercise>,
    current_goal_index: usize,
    completed_goals: Vec<bool>,
    last_state: Option<VimState>,
    monitoring_active: bool,
    instruction_pane_id: Option<String>,
}

impl ContinuousVimSession {
    pub fn new(socket_path: String) -> Self {
        Self {
            vim_client: VimRpcClient::new(socket_path),
            goal_detector: GoalDetector::new(),
            current_exercise: None,
            current_goal_index: 0,
            completed_goals: Vec::new(),
            last_state: None,
            monitoring_active: false,
            instruction_pane_id: None,
        }
    }

    pub fn start_exercise(&mut self, exercise: ContinuousExercise, file_path: &str) -> Result<()> {
        println!("\n🎯 === {} ===", exercise.title);
        println!("{}\n", exercise.description);

        // サンプルコードを表示
        println!("📝 サンプルコード:");
        for (i, line) in exercise.sample_code.iter().enumerate() {
            println!("{:2}: {}", i + 1, line);
        }
        println!();

        // 目標リストを表示
        println!("🎯 学習目標:");
        for (i, goal) in exercise.goals.iter().enumerate() {
            println!("  {}. {}", i + 1, goal.description);
            if let Some(hint) = &goal.hint {
                println!("     💡 ヒント: {}", hint);
            }
        }
        println!();

        // tmux分割画面でVimを起動
        if Command::new("tmux").arg("-V").output().is_ok() {
            println!("🖥️ tmux分割画面モードで学習を開始します");
            self.start_tmux_session(&exercise, file_path)?;
        } else {
            println!("❌ tmuxが利用できません。RPCモードで実行します");
            // fallback to RPC mode
            self.vim_client.start_neovim(file_path, None)?;
            thread::sleep(Duration::from_millis(500));
        }

        // 練習の初期化
        self.current_exercise = Some(exercise.clone());
        self.completed_goals = vec![false; exercise.goals.len()];
        self.current_goal_index = 0;
        self.monitoring_active = true;

        debug_log!("🚀 Vimセッション開始！");
        debug_log!("現在の目標: {}", exercise.goals[0].description);

        Ok(())
    }

    fn start_tmux_session(&mut self, exercise: &ContinuousExercise, file_path: &str) -> Result<()> {
        let session_name = "vim_tutorial_continuous";

        // 既存セッションを削除
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", session_name])
            .output();

        // 新しいセッションを作成
        let new_session_result = Command::new("tmux")
            .args(["new-session", "-d", "-s", session_name])
            .output()?;

        if !new_session_result.status.success() {
            return Err(anyhow::anyhow!(
                "tmuxセッション作成に失敗: {}",
                String::from_utf8_lossy(&new_session_result.stderr)
            ));
        }

        // 画面を水平分割
        let split_result = Command::new("tmux")
            .args(["split-window", "-v", "-t", session_name])
            .output()?;

        if !split_result.status.success() {
            return Err(anyhow::anyhow!(
                "tmux画面分割に失敗: {}",
                String::from_utf8_lossy(&split_result.stderr)
            ));
        }
        
        // 分割後にペイン一覧を取得して正確なIDを確認
        let pane_list_output = Command::new("tmux")
            .args(["list-panes", "-t", session_name, "-F", "#{pane_index}:#{pane_id}:#{pane_current_command}"])
            .output()?;
        
        let pane_info = String::from_utf8_lossy(&pane_list_output.stdout);
        debug_log!("分割後ペイン一覧: {}", pane_info.trim());
        
        // pane_index 0 = 上部（指示用）、pane_index 1 = 下部（Vim用）
        let mut top_pane_id = String::new();
        let mut bottom_pane_id = String::new();
        
        for line in pane_info.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 {
                let index = parts[0];
                let pane_id = parts[1];
                // index 0 = 上部（指示表示用）
                // index 1 = 下部（Vim用）
                if index == "0" {
                    top_pane_id = pane_id.to_string();
                } else if index == "1" {
                    bottom_pane_id = pane_id.to_string();
                }
            }
        }
        
        debug_log!("上部ペインID: {}", top_pane_id);
        debug_log!("下部ペインID: {}", bottom_pane_id);

        // ペイン識別のためのテストメッセージ送信
        if !top_pane_id.is_empty() {
            let _ = Command::new("tmux")
                .args(["send-keys", "-t", &top_pane_id, "echo 'TEST: 上部ペイン'", "Enter"])
                .output();
            debug_log!("上部ペインにテストメッセージ送信: {}", top_pane_id);
        }
        
        if !bottom_pane_id.is_empty() {
            let _ = Command::new("tmux")
                .args(["send-keys", "-t", &bottom_pane_id, "echo 'TEST: 下部ペイン'", "Enter"])
                .output();
            debug_log!("下部ペインにテストメッセージ送信: {}", bottom_pane_id);
        }
        
        thread::sleep(Duration::from_millis(1000)); // テストメッセージを確認するための待機

        // instruction_pane_idを保存
        self.instruction_pane_id = Some(top_pane_id.clone());
        
        // 取得したペインIDを使用
        let top_pane = &top_pane_id;    // 上部ペイン（指示）
        let bottom_pane = &bottom_pane_id; // 下部ペイン（Vim）

        // Vimスクリプトを作成
        let vim_script = self.create_vim_script()?;

        // 上部ペインに指示を表示
        let instruction_command = self.create_instruction_command(exercise)?;

        debug_log!("上部ペイン({})に指示送信: {}", top_pane, instruction_command.chars().take(100).collect::<String>());
        let instruction_result = Command::new("tmux")
            .args(["send-keys", "-t", top_pane, &instruction_command, "Enter"])
            .output();
        debug_log!("指示送信結果: success={}", instruction_result.as_ref().map(|r| r.status.success()).unwrap_or(false));

        thread::sleep(Duration::from_millis(200));

        // 下部ペインでVimを起動
        let nvim_command = format!(
            "nvim -S {} {}; tmux detach-client",
            vim_script.path().display(),
            file_path
        );

        debug_log!("下部ペイン({})にVimコマンド送信: {}", bottom_pane, nvim_command);
        let vim_result = Command::new("tmux")
            .args(["send-keys", "-t", bottom_pane, &nvim_command, "Enter"])
            .output();
        debug_log!("Vim起動結果: success={}", vim_result.as_ref().map(|r| r.status.success()).unwrap_or(false));

        thread::sleep(Duration::from_millis(500));

        // 下部ペインにフォーカス
        let _ = Command::new("tmux")
            .args(["select-pane", "-t", bottom_pane])
            .output();

        // 現在のターミナルをtmuxセッションにアタッチ
        println!("🖥️ 画面を分割してVim学習を開始します...");
        println!("💡 操作方法:");
        println!("   - 上部: 指示とリアルタイム進捗表示");
        println!("   - 下部: Vim操作画面");
        println!("   - Ctrl+b ↑/↓でペイン間移動可能");
        println!();

        // 少し待ってから現在のターミナルでtmuxセッションをアタッチ
        thread::sleep(Duration::from_millis(1000));

        debug_log!("tmuxアタッチ準備完了、Vimセッション開始！");
        
        // tmuxセッションにアタッチ（非同期で実行）
        println!("🖥️ tmuxセッションにアタッチ中...");
        println!("💡 操作方法:");
        println!("   - 上部: 指示とリアルタイム進捗表示");
        println!("   - 下部: Vim操作画面");
        println!("   - Ctrl+b ↑/↓でペイン間移動可能");
        println!("   - 目標達成後、自動的に次の目標に進みます");
        println!();

        let session_name_clone = session_name.to_string();
        thread::spawn(move || {
            debug_log!("tmuxアタッチスレッド開始");
            let attach_result = Command::new("tmux")
                .args(["attach-session", "-t", &session_name_clone])
                .status();

            match attach_result {
                Ok(_) => {
                    debug_log!("tmuxセッション正常終了");
                }
                Err(e) => {
                    debug_log!("tmuxアタッチエラー: {}", e);
                }
            }
        });

        // 短時間待機してから戻る（監視スレッドを開始できるように）
        thread::sleep(Duration::from_millis(500));

        Ok(())
    }

    fn create_vim_script(&self) -> Result<NamedTempFile> {
        let script_content = r#"
" 連続学習用Vimスクリプト（拡張版）
function! UpdateStatus()
  let line_num = line('.')
  let col_num = col('.')
  let mode_str = mode()
  let mode_detailed = mode(1)
  let status_line = 'LINE:' . line_num . ',COL:' . col_num . ',MODE:' . mode_str . ',DETAILED:' . mode_detailed
  call writefile([status_line], '/tmp/vim_continuous_status.json')
endfunction

" 複数の状態更新トリガー
autocmd CursorMoved,CursorMovedI,InsertEnter,InsertLeave,ModeChanged * call UpdateStatus()

" タイマーベースの定期更新（100ms間隔）
function! TimerUpdate(timer)
  call UpdateStatus()
endfunction

let g:update_timer = timer_start(100, 'TimerUpdate', {'repeat': -1})

" 基本移動キーの即座更新マッピング
for key in ['h', 'j', 'k', 'l', 'w', 'e', 'b', '0', '$', 'gg', 'G']
  execute 'nnoremap <silent> ' . key . ' ' . key . ':call UpdateStatus()<CR>'
endfor

" 初期状態を記録
call UpdateStatus()

" カーソル位置を1行1列に設定
call cursor(1, 1)
call UpdateStatus()

" echo '🎯 連続学習開始！リアルタイム状態監視が有効です'
"#;

        let script_file = NamedTempFile::new()?;
        fs::write(&script_file, script_content)?;
        Ok(script_file)
    }

    fn create_instruction_command(&self, exercise: &ContinuousExercise) -> Result<String> {
        let success_flag = "/tmp/vim_continuous_success.flag";
        let progress_flag = "/tmp/vim_continuous_progress.txt";
        let _ = fs::remove_file(success_flag);
        let _ = fs::remove_file(progress_flag);

        // 最初の目標だけを表示
        let first_goal = &exercise.goals[0];
        let goal_display = format!("  1. {}", first_goal.description.replace("'", "'\\''"));
        let hint_display = if let Some(hint) = &first_goal.hint {
            format!("     💡 {}", hint.replace("'", "'\\''"))
        } else {
            String::new()
        };

        // シンプルな指示表示（複雑なbashループは削除）
        let command = format!(
            r#"clear; echo '=== 🎯 {} ==='; echo '{}'; echo ''; echo '=== 📋 現在の目標 ==='; echo '{}'; echo '{}'; echo '=== 📊 進捗: 1/{} ==='; echo '下のNeovimで操作してください！'; echo '目標達成時に自動的に次の目標が表示されます'"#,
            exercise.title.replace("'", "'\\''"),
            exercise.description.replace("'", "'\\''"),
            goal_display,
            hint_display,
            exercise.goals.len()
        );

        Ok(command)
    }

    pub fn monitor_progress(&mut self) -> Result<ExerciseResult> {
        let status_file = "/tmp/vim_continuous_status.json";
        // let success_flag = "/tmp/vim_continuous_success.flag";
        let progress_flag = "/tmp/vim_continuous_progress.txt";

        debug_log!("監視開始: status_file={}", status_file);

        while self.monitoring_active {
            thread::sleep(Duration::from_millis(100));

            // ステータスファイルから現在の状態を読み取り
            let current_state = self.read_vim_state_from_file(status_file)?;
            debug_log!("現在の状態: line={}, col={}, mode={:?}", 
                      current_state.cursor_line, current_state.cursor_col, current_state.mode);

            if let Some(exercise) = self.current_exercise.clone() {
                // 現在のゴールをチェック
                if self.current_goal_index < exercise.goals.len() {
                    let current_goal_def = &exercise.goals[self.current_goal_index];
                    let goal = self.convert_goal_definition(current_goal_def)?;
                    
                    debug_log!("目標チェック中: goal_index={}, goal_type={:?}", 
                              self.current_goal_index, goal.goal_type);

                    let goal_achieved = self.goal_detector.check_goal(&goal, &current_state);
                    debug_log!("目標達成判定: {}", goal_achieved);

                    if goal_achieved {
                        // 現在の目標を達成
                        self.completed_goals[self.current_goal_index] = true;
                        self.current_goal_index += 1;

                        debug_log!("✅ 目標達成: {}", current_goal_def.description);

                        if self.current_goal_index >= exercise.goals.len() {
                            // 全ての目標を完了
                            if let Ok(mut file) = OpenOptions::new()
                                .create(true)
                                .write(true)
                                .truncate(true)
                                .open(progress_flag)
                            {
                                let _ = writeln!(file, "completed");
                            }
                            debug_log!("🎉 全ての目標を達成しました！");
                            
                            // 章完了時にメニューに戻る
                            self.show_completion_message(&exercise)?;
                            thread::sleep(Duration::from_millis(2000));
                            
                            return Ok(ExerciseResult::Completed);
                        } else {
                            // 次の目標に進む
                            if let Ok(mut file) = OpenOptions::new()
                                .create(true)
                                .write(true)
                                .truncate(true)
                                .open(progress_flag)
                            {
                                let _ = writeln!(file, "{}", self.current_goal_index + 1);
                            }

                            // 上部ペインを更新（新しい目標を表示）
                            self.update_instruction_pane(&exercise)?;

                            debug_log!(
                                "📍 次の目標: {}",
                                exercise.goals[self.current_goal_index].description
                            );
                        }

                        // 少し待ってから進捗を反映
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            }

            self.last_state = Some(current_state);
        }

        Ok(ExerciseResult::Incomplete)
    }

    fn update_instruction_pane(&self, exercise: &ContinuousExercise) -> Result<()> {
        // 保存されたペインIDを使用
        let top_pane = match &self.instruction_pane_id {
            Some(pane_id) => {
                debug_log!("保存されたペインIDを使用: {}", pane_id);
                pane_id
            },
            None => {
                debug_log!("instruction_pane_id が設定されていません");
                return Err(anyhow::anyhow!("instruction_pane_id が設定されていません"));
            }
        };
        let current_goal = &exercise.goals[self.current_goal_index];
        let goal_display = format!(
            "  {}. {}",
            self.current_goal_index + 1,
            current_goal.description.replace("'", "'\\''")
        );
        let hint_display = if let Some(hint) = &current_goal.hint {
            format!("     💡 {}", hint.replace("'", "'\\''"))
        } else {
            String::new()
        };

        let update_command = format!(
            "clear; echo '=== 🎯 {} ==='; echo '{}'; echo ''; echo '=== 📋 現在の目標 ==='; echo '{}'; echo '{}'; echo '=== 📊 進捗: {}/{} ==='; echo '下のNeovimで操作してください！'",
            exercise.title.replace("'", "'\\''"),
            exercise.description.replace("'", "'\\''"),
            goal_display,
            hint_display,
            self.current_goal_index + 1,
            exercise.goals.len()
        );

        // 上部ペインの内容を更新
        debug_log!("上部ペイン({})を更新: {}", top_pane, update_command.chars().take(100).collect::<String>());
        let interrupt_result = Command::new("tmux")
            .args(["send-keys", "-t", top_pane, "C-c"]) // 現在のコマンドを中断
            .output();
        debug_log!("中断送信結果: success={}", interrupt_result.as_ref().map(|r| r.status.success()).unwrap_or(false));

        thread::sleep(Duration::from_millis(100));

        let update_result = Command::new("tmux")
            .args(["send-keys", "-t", top_pane, &update_command, "Enter"])
            .output();
        debug_log!("更新送信結果: success={}", update_result.as_ref().map(|r| r.status.success()).unwrap_or(false));

        Ok(())
    }

    fn read_vim_state_from_file(&self, status_file: &str) -> Result<VimState> {
        debug_log!("状態ファイル読み取り: {}", status_file);
        
        // ファイルが存在しない場合はデフォルト状態を返す
        let content = match fs::read_to_string(status_file) {
            Ok(content) => {
                debug_log!("ファイル内容: {}", content.trim());
                content
            },
            Err(e) => {
                debug_log!("ファイル読み取りエラー: {}", e);
                // デフォルト状態
                return Ok(VimState {
                    mode: VimMode::Normal,
                    cursor_line: 0,
                    cursor_col: 0,
                    operator: None,
                    buffer_content: vec!["".to_string()],
                    registers: std::collections::HashMap::new(),
                });
            }
        };

        // 状態ファイルから情報をパース
        // 形式: "LINE:1,COL:1,MODE:n,DETAILED:n"
        let mut line_num = 1;
        let mut col_num = 1;
        let mut mode_str = "n".to_string();
        let mut mode_detailed = "n".to_string();

        for line in content.lines() {
            if line.starts_with("LINE:") {
                let parts: Vec<&str> = line.split(',').collect();
                for part in parts {
                    if let Some(value) = part.strip_prefix("LINE:") {
                        line_num = value.parse().unwrap_or(1);
                    } else if let Some(value) = part.strip_prefix("COL:") {
                        col_num = value.parse().unwrap_or(1);
                    } else if let Some(value) = part.strip_prefix("MODE:") {
                        mode_str = value.to_string();
                    } else if let Some(value) = part.strip_prefix("DETAILED:") {
                        mode_detailed = value.to_string();
                    }
                }
                break;
            }
        }

        let vim_mode = VimMode::from_vim_mode(&mode_str, &mode_detailed, None);

        let final_state = VimState {
            mode: vim_mode,
            cursor_line: (line_num - 1) as usize, // Vimは1ベース、内部は0ベース
            cursor_col: (col_num - 1) as usize,
            operator: None,
            buffer_content: vec!["".to_string()], // 簡略化
            registers: std::collections::HashMap::new(),
        };

        debug_log!("パース結果: line_num={} -> {}, col_num={} -> {}, mode={}",
                  line_num, final_state.cursor_line, col_num, final_state.cursor_col, mode_str);

        Ok(final_state)
    }

    // fn check_goals(
    //     &mut self,
    //     current_state: &VimState,
    //     exercise: &ContinuousExercise,
    // ) -> Result<Option<ExerciseResult>> {
    //     match exercise.flow_type {
    //         FlowType::Sequential => self.check_sequential_goals(current_state, exercise),
    //         FlowType::AnyOrder => self.check_any_order_goals(current_state, exercise),
    //         FlowType::Parallel => self.check_parallel_goals(current_state, exercise),
    //     }
    // }

    // fn check_sequential_goals(
    //     &mut self,
    //     current_state: &VimState,
    //     exercise: &ContinuousExercise,
    // ) -> Result<Option<ExerciseResult>> {
    //     if self.current_goal_index >= exercise.goals.len() {
    //         return Ok(Some(ExerciseResult::Completed));
    //     }

    //     let current_goal_def = &exercise.goals[self.current_goal_index];
    //     let goal = self.convert_goal_definition(current_goal_def)?;

    //     if self.goal_detector.check_goal(&goal, current_state) {
    //         println!("✅ 目標達成: {}", current_goal_def.description);
    //         self.completed_goals[self.current_goal_index] = true;
    //         self.current_goal_index += 1;

    //         if self.current_goal_index >= exercise.goals.len() {
    //             println!("\n🎉 全ての目標を達成しました！");
    //             return Ok(Some(ExerciseResult::Completed));
    //         } else {
    //             println!(
    //                 "📍 次の目標: {}",
    //                 exercise.goals[self.current_goal_index].description
    //             );
    //             if let Some(hint) = &exercise.goals[self.current_goal_index].hint {
    //                 println!("💡 ヒント: {}", hint);
    //             }
    //         }
    //     }

    //     Ok(None)
    // }

    // fn check_any_order_goals(
    //     &mut self,
    //     current_state: &VimState,
    //     exercise: &ContinuousExercise,
    // ) -> Result<Option<ExerciseResult>> {
    //     let mut progress_made = false;

    //     for (i, goal_def) in exercise.goals.iter().enumerate() {
    //         if self.completed_goals[i] {
    //             continue; // 既に完了している目標はスキップ
    //         }

    //         let goal = self.convert_goal_definition(goal_def)?;
    //         if self.goal_detector.check_goal(&goal, current_state) {
    //             println!("✅ 目標達成: {}", goal_def.description);
    //             self.completed_goals[i] = true;
    //             progress_made = true;
    //         }
    //     }

    //     // 全ての目標が完了したかチェック
    //     if self.completed_goals.iter().all(|&completed| completed) {
    //         println!("\n🎉 全ての目標を達成しました！");
    //         return Ok(Some(ExerciseResult::Completed));
    //     }

    //     if progress_made {
    //         self.show_remaining_goals(exercise);
    //     }

    //     Ok(None)
    // }

    // fn check_parallel_goals(
    //     &mut self,
    //     current_state: &VimState,
    //     exercise: &ContinuousExercise,
    // ) -> Result<Option<ExerciseResult>> {
    //     // 並列目標：全ての目標を同時に満たす必要がある
    //     let mut all_satisfied = true;

    //     for goal_def in &exercise.goals {
    //         let goal = self.convert_goal_definition(goal_def)?;
    //         if !self.goal_detector.check_goal(&goal, current_state) {
    //             all_satisfied = false;
    //             break;
    //         }
    //     }

    //     if all_satisfied {
    //         println!("\n🎉 全ての目標を同時に達成しました！");
    //         return Ok(Some(ExerciseResult::Completed));
    //     }

    //     Ok(None)
    // }

    fn convert_goal_definition(&self, goal_def: &ExerciseGoal) -> Result<Goal> {
        debug_log!("目標変換: type={}, target={:?}", goal_def.goal_type, goal_def.target);
        
        let goal_type = match goal_def.goal_type.as_str() {
            "position" => {
                let target = goal_def
                    .target
                    .as_array()
                    .ok_or_else(|| anyhow::anyhow!("Position target must be an array"))?;
                let line = target[0].as_u64().unwrap_or(0) as usize;
                let col = target[1].as_u64().unwrap_or(0) as usize;
                debug_log!("Position目標: line={}, col={}", line, col);
                GoalType::Position { line, col }
            }
            "mode" => {
                let mode_str = goal_def
                    .target
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Mode target must be a string"))?;
                let vim_mode = match mode_str {
                    "normal" => VimMode::Normal,
                    "insert" => VimMode::Insert,
                    "visual" => VimMode::Visual,
                    "visual_line" => VimMode::VisualLine,
                    "visual_block" => VimMode::VisualBlock,
                    "command" => VimMode::Command,
                    op if op.starts_with("operator_") => {
                        let operator = op.strip_prefix("operator_").unwrap_or("");
                        VimMode::OperatorPending(operator.to_string())
                    }
                    _ => return Err(anyhow::anyhow!("Unknown mode: {}", mode_str)),
                };
                GoalType::Mode(vim_mode)
            }
            "text" => {
                let target = goal_def
                    .target
                    .as_object()
                    .ok_or_else(|| anyhow::anyhow!("Text target must be an object"))?;
                let line = target["line"].as_u64().unwrap_or(0) as usize;
                let expected = target["expected"].as_str().unwrap_or("").to_string();
                GoalType::TextContent { line, expected }
            }
            "register" => {
                let target = goal_def
                    .target
                    .as_object()
                    .ok_or_else(|| anyhow::anyhow!("Register target must be an object"))?;
                let register = target["register"].as_str().unwrap_or("").to_string();
                let expected = target["expected"].as_str().unwrap_or("").to_string();
                GoalType::RegisterContent { register, expected }
            }
            "buffer_change" => GoalType::BufferChange,
            _ => return Err(anyhow::anyhow!("Unknown goal type: {}", goal_def.goal_type)),
        };

        Ok(Goal {
            goal_type,
            description: goal_def.description.clone(),
        })
    }

    // fn show_remaining_goals(&self, exercise: &ContinuousExercise) {
    //     println!("📋 残りの目標:");
    //     for (i, goal_def) in exercise.goals.iter().enumerate() {
    //         if !self.completed_goals[i] {
    //             println!("  • {}", goal_def.description);
    //         }
    //     }
    //     println!();
    // }

    fn show_completion_message(&self, exercise: &ContinuousExercise) -> Result<()> {
        if let Some(pane_id) = &self.instruction_pane_id {
            let completion_command = format!(
                "clear; echo '=== 🎉 章完了！ ==='; echo '{}'; echo ''; echo '✅ 全ての目標を達成しました！'; echo ''; echo '📋 達成した目標:'; {} echo '';",
                exercise.title.replace("'", "'\\''"),
                exercise.goals.iter().enumerate().map(|(i, goal)| 
                    format!("echo '  {}. {}'", i + 1, goal.description.replace("'", "'\\''"))
                ).collect::<Vec<_>>().join("; ")
            );
            
            let _ = Command::new("tmux")
                .args(["send-keys", "-t", pane_id, &completion_command, "Enter"])
                .output();
            
            debug_log!("完了メッセージ表示: {}", pane_id);
        }
        Ok(())
    }

    pub fn stop_exercise(&mut self) -> Result<()> {
        self.monitoring_active = false;

        // tmuxセッションをクリーンアップ
        let session_name = "vim_tutorial_continuous";
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", session_name])
            .output();

        // 状態ファイルをクリーンアップ
        let _ = fs::remove_file("/tmp/vim_continuous_status.json");
        let _ = fs::remove_file("/tmp/vim_continuous_success.flag");

        // RPC クライアントも停止
        self.vim_client.stop()?;

        println!("📱 セッションを終了しました");
        Ok(())
    }

    // pub fn send_keys(&self, keys: &str) -> Result<()> {
    //     self.vim_client.send_keys(keys)
    // }

    // pub fn get_current_state(&self) -> Result<VimState> {
    //     self.vim_client.get_current_state()
    // }
}

#[derive(Debug, PartialEq)]
pub enum ExerciseResult {
    Completed,
    Incomplete,
    #[allow(dead_code)] // エラーハンドリング用
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    // use std::fs;
    use tempfile::tempdir;

    // fn create_test_exercise() -> ContinuousExercise {
    //     ContinuousExercise {
    //         title: "Test Exercise".to_string(),
    //         description: "A test exercise".to_string(),
    //         sample_code: vec!["hello world".to_string()],
    //         goals: vec![
    //             ExerciseGoal {
    //                 goal_type: "position".to_string(),
    //                 target: json!([0, 5]),
    //                 description: "Move to position 0,5".to_string(),
    //                 hint: Some("Use 'l' key to move right".to_string()),
    //             },
    //             ExerciseGoal {
    //                 goal_type: "mode".to_string(),
    //                 target: json!("insert"),
    //                 description: "Enter insert mode".to_string(),
    //                 hint: None,
    //             },
    //         ],
    //         flow_type: FlowType::Sequential,
    //     }
    // }

    #[test]
    fn test_continuous_session_creation() {
        let session = ContinuousVimSession::new("/tmp/test.sock".to_string());
        assert!(session.current_exercise.is_none());
        assert_eq!(session.current_goal_index, 0);
    }

    #[test]
    fn test_goal_conversion() -> Result<()> {
        let tmp_dir = tempdir()?;
        let socket_path = tmp_dir
            .path()
            .join("test.sock")
            .to_string_lossy()
            .to_string();
        let session = ContinuousVimSession::new(socket_path);

        // Position goal
        let pos_goal_def = ExerciseGoal {
            goal_type: "position".to_string(),
            target: json!([1, 2]),
            description: "Test position".to_string(),
            hint: None,
        };
        let goal = session.convert_goal_definition(&pos_goal_def)?;
        match goal.goal_type {
            GoalType::Position { line, col } => {
                assert_eq!(line, 1);
                assert_eq!(col, 2);
            }
            _ => panic!("Expected Position goal type"),
        }

        // Mode goal
        let mode_goal_def = ExerciseGoal {
            goal_type: "mode".to_string(),
            target: json!("insert"),
            description: "Test mode".to_string(),
            hint: None,
        };
        let goal = session.convert_goal_definition(&mode_goal_def)?;
        match goal.goal_type {
            GoalType::Mode(VimMode::Insert) => {}
            _ => panic!("Expected Insert mode goal type"),
        }

        Ok(())
    }

    #[test]
    fn test_operator_pending_goal_conversion() -> Result<()> {
        let tmp_dir = tempdir()?;
        let socket_path = tmp_dir
            .path()
            .join("test.sock")
            .to_string_lossy()
            .to_string();
        let session = ContinuousVimSession::new(socket_path);

        let op_goal_def = ExerciseGoal {
            goal_type: "mode".to_string(),
            target: json!("operator_d"),
            description: "Press 'd' for delete".to_string(),
            hint: None,
        };

        let goal = session.convert_goal_definition(&op_goal_def)?;
        match goal.goal_type {
            GoalType::Mode(VimMode::OperatorPending(op)) => {
                assert_eq!(op, "d");
            }
            _ => panic!("Expected OperatorPending goal type"),
        }

        Ok(())
    }
}
