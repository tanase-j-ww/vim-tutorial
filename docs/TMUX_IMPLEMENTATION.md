# tmux分割画面実装の技術ドキュメント

## 概要
Vim Tutorial Gameにおけるtmux分割画面機能の実装で得られた知見と、遭遇した問題とその解決方法をまとめます。

## 実装の目的
- 上部ペイン：学習指示とリアルタイム監視
- 下部ペイン：実際のNeovim操作画面
- 両ペインの同時表示により、指示を見ながら操作を学習可能

## 主要な技術的課題と解決方法

### 1. ペインの消失問題

#### 問題
`exec </dev/null` を使用すると、シェルが終了してペインが閉じてしまう。

#### 原因
`exec` コマンドは現在のシェルプロセスを置き換えるため、標準入力を `/dev/null` にリダイレクトすると、シェルが即座に終了する。

#### 解決方法
```bash
# ❌ 問題のあるコード
echo "指示内容"; exec </dev/null

# ✅ 解決方法1: catを使用（Ctrl+Cで中断可能）
echo "指示内容"; cat

# ✅ 解決方法2: sleep infinityを使用（無限待機）
echo "指示内容"; sleep infinity

# ✅ 解決方法3: whileループで状態監視
while true; do
  if [ -f /tmp/success.flag ]; then
    echo "成功メッセージ"
    break
  fi
  sleep 0.2
done
```

### 2. ペインID取得の問題

#### 問題
tmuxのペイン番号（0, 1）とペインID（%0, %1）の混同により、コマンド送信が失敗。

#### 解決方法
```rust
// ペイン一覧を取得してIDを解析
let pane_list_result = Command::new("tmux")
    .args(["list-panes", "-t", session_name, "-F", "#{pane_index}:#{pane_id}"])
    .output()?;

// "0:%0\n1:%1" のような出力から %0, %1 を抽出
let panes: Vec<String> = pane_list
    .trim()
    .lines()
    .map(|line| {
        line.split(':').nth(1).unwrap_or("").to_string()
    })
    .collect();

let top_pane = &panes[0];    // "%0"
let bottom_pane = &panes[1];  // "%1"
```

### 3. デバッグログの表示干渉

#### 問題
`println!` による画面出力がVimの表示と干渉し、画面が崩れる。

#### 解決方法
```rust
// ファイルのみにログ出力
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
```

### 4. 動的ステータス更新の問題

#### 問題
上部ペインで動的にステータスを更新しようとすると、Ctrl+Cによる中断で表示が崩れる。

#### 初期実装（問題あり）
```bash
while true; do
  clear
  echo "現在のカーソル位置: $(cat /tmp/status)"
  sleep 1
done
```

#### 改善実装
成功フラグファイルを使用した静的表示と成功時の切り替え：
```bash
echo "初期表示"
while true; do
  if [ -f /tmp/success.flag ]; then
    clear
    echo "成功メッセージ"
    rm /tmp/success.flag
    break
  fi
  sleep 0.2
done
```

### 5. tmuxセッションのクリーンアップ

#### 問題
セッション終了後も分割画面が残り、ユーザーが手動で終了する必要があった。

#### 解決方法
```rust
// セッション終了後の確実なクリーンアップ
let cleanup_result = Command::new("tmux")
    .args(["kill-session", "-t", session_name])
    .output();

// 状態ファイルの削除
let _ = fs::remove_file(status_file);
let _ = fs::remove_file("/tmp/vim_tutorial_success.flag");

// ターミナルをクリア（元の画面に戻す）
print!("\x1b[2J\x1b[H"); // 画面クリア + カーソルを左上に移動
io::stdout().flush().unwrap_or(());
```

## 成功監視システムの実装

### バックグラウンド監視スレッド
```rust
fn monitor_neovim_status(status_file: &str, step: StepData, _top_pane: &str) {
    let mut success_triggered = false;
    let target_position = if let Some(cursor_end) = step.cursor_end {
        (cursor_end[0] as i32 + 1, cursor_end[1] as i32 + 1)
    } else {
        return;
    };
    
    loop {
        if let Ok(content) = fs::read_to_string(status_file) {
            // カーソル位置を解析
            let current_position = parse_cursor_position(&content);
            
            // 目標達成時の処理
            if current_position == target_position && !success_triggered {
                // 成功フラグファイルを作成
                if let Ok(mut file) = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .open("/tmp/vim_tutorial_success.flag") {
                    let _ = writeln!(file, "SUCCESS");
                }
                success_triggered = true;
            }
        }
        
        thread::sleep(Duration::from_millis(200));
    }
}
```

## tmuxコマンドのベストプラクティス

### 1. セッション作成と分割
```bash
# セッション作成
tmux new-session -d -s session_name

# 画面分割（水平）
tmux split-window -v -t session_name

# ペインの高さ調整（上12行、下11行）
tmux resize-pane -t %0 -y 12
```

### 2. コマンド送信
```bash
# ペインへのコマンド送信
tmux send-keys -t %0 "echo 'Hello'" Enter

# 複雑なコマンドはbash -cでラップ
tmux send-keys -t %0 'bash -c "複雑なコマンド"' Enter
```

### 3. セッション監視
```bash
# ペイン一覧の取得
tmux list-panes -t session_name -F "#{pane_index}:#{pane_id}:#{pane_active}"

# セッション存在確認
tmux has-session -t session_name 2>/dev/null
```

## トラブルシューティング

### デバッグ時のチェックポイント
1. **ペインの状態確認**
   ```bash
   tmux list-panes -t vim_tutorial
   ```

2. **ログファイルの監視**
   ```bash
   tail -f /tmp/vim_tutorial_debug.log
   ```

3. **プロセス確認**
   ```bash
   ps aux | grep tmux
   ```

4. **手動テスト**
   ```bash
   # 手動でtmuxコマンドを実行して動作確認
   tmux new-session -d -s test
   tmux split-window -v -t test
   tmux send-keys -t test:0.0 "echo 'Top pane'" Enter
   tmux send-keys -t test:0.1 "echo 'Bottom pane'" Enter
   tmux attach-session -t test
   ```

### よくある問題と対処法

| 問題 | 原因 | 対処法 |
|------|------|--------|
| ペインが表示されない | `exec </dev/null` によるシェル終了 | `sleep infinity` を使用 |
| コマンドが実行されない | ペインIDの誤り | `%0`, `%1` 形式を使用 |
| 画面が崩れる | デバッグログの画面出力 | ファイルのみにログ出力 |
| セッションが残る | クリーンアップ不足 | `kill-session` を確実に実行 |
| 成功メッセージが見えない | 動的更新の問題 | フラグファイル監視方式を採用 |

## パフォーマンス考慮事項

1. **監視間隔**: 200msごとのポーリングで十分な応答性を確保
2. **ファイルI/O**: 状態ファイルへのアクセスを最小限に
3. **プロセス起動**: tmuxコマンドの実行回数を最適化

## 今後の改善案

1. **tmux以外の方法の検討**
   - PTY（擬似端末）を直接制御
   - 別ウィンドウでの表示

2. **エラーハンドリングの強化**
   - tmuxがインストールされていない場合の代替手段
   - セッション作成失敗時のリトライ機構

3. **ユーザビリティの向上**
   - ペインサイズの動的調整
   - カラー表示の改善
   - プログレスバーの追加

## まとめ

tmux分割画面の実装において最も重要なのは：
1. ペインの生存維持（`sleep infinity`の使用）
2. 正確なペインID管理（`%0`, `%1`形式）
3. クリーンなログ出力（ファイルのみ）
4. 確実なクリーンアップ処理
5. フラグファイルを使用した状態通信

これらの知見により、安定した分割画面学習環境を実現できました。