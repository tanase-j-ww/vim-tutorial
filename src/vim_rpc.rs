use crate::vim_state::{VimMode, VimState};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub struct VimRpcClient {
    socket_path: String,
    nvim_process_id: Option<u32>,
}

impl VimRpcClient {
    pub fn new(socket_path: String) -> Self {
        Self {
            socket_path,
            nvim_process_id: None,
        }
    }

    pub fn start_neovim(&mut self, file_path: &str, script_path: Option<&str>) -> Result<()> {
        // 既存のソケットファイルを削除
        if Path::new(&self.socket_path).exists() {
            std::fs::remove_file(&self.socket_path)?;
        }

        // Neovimを起動
        let mut cmd = Command::new("nvim");
        cmd.arg("--headless").arg("--listen").arg(&self.socket_path);

        if let Some(script) = script_path {
            cmd.arg("-S").arg(script);
        }

        cmd.arg(file_path);

        let child = cmd.spawn()?;
        self.nvim_process_id = Some(child.id());

        // Neovimが起動するまで待機
        let mut retries = 10;
        while retries > 0 && !Path::new(&self.socket_path).exists() {
            std::thread::sleep(std::time::Duration::from_millis(100));
            retries -= 1;
        }

        if !Path::new(&self.socket_path).exists() {
            return Err(anyhow!("Failed to start Neovim: socket not created"));
        }

        Ok(())
    }

    #[allow(unused)]
    pub fn send_keys(&self, keys: &str) -> Result<()> {
        let output = Command::new("nvim")
            .args(["--server", &self.socket_path, "--remote-send", keys])
            .output()?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to send keys '{}': {}",
                keys,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    #[allow(unused)]
    pub fn get_current_state(&self) -> Result<VimState> {
        // 複数の情報を並行して取得
        let mode = self.eval_expr("mode()")?;
        let mode_detailed = self.eval_expr("mode(1)")?;
        let line = self.eval_expr("line('.')")?.parse::<usize>().unwrap_or(1);
        let col = self.eval_expr("col('.')")?.parse::<usize>().unwrap_or(1);

        // オペレーターの取得
        let operator = match self.eval_expr("exists('v:operator') ? v:operator : ''") {
            Ok(op) if !op.is_empty() => Some(op),
            _ => None,
        };

        // バッファ内容の取得
        let buffer_lines_str = self.eval_expr("join(getline(1,'$'), '\\n')")?;
        let buffer_content: Vec<String> = buffer_lines_str
            .split('\n')
            .map(|s| s.to_string())
            .collect();

        // レジスタ情報の取得
        let mut registers = HashMap::new();
        for reg in &["\"", "0", "1", "a", "b", "c"] {
            if let Ok(content) = self.eval_expr(&format!("@{}", reg))
                && !content.is_empty()
            {
                registers.insert(reg.to_string(), content);
            }
        }

        let vim_mode = VimMode::from_vim_mode(&mode, &mode_detailed, operator.clone());

        Ok(VimState {
            mode: vim_mode,
            cursor_line: line.saturating_sub(1), // Vim は1ベース、内部は0ベース
            cursor_col: col.saturating_sub(1),
            operator,
            buffer_content,
            registers,
        })
    }

    #[allow(unused)]
    pub fn eval_expr(&self, expr: &str) -> Result<String> {
        let output = Command::new("nvim")
            .args(["--server", &self.socket_path, "--remote-expr", expr])
            .output()?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to evaluate expression '{}': {}",
                expr,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let result = String::from_utf8_lossy(&output.stdout);
        Ok(result.trim().to_string())
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(pid) = self.nvim_process_id {
            // プロセスを終了
            if let Ok(mut child) = std::process::Command::new("kill")
                .arg(pid.to_string())
                .spawn()
            {
                let _ = child.wait();
            }
            self.nvim_process_id = None;
        }

        // ソケットファイルをクリーンアップ
        if Path::new(&self.socket_path).exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }

        Ok(())
    }
}

impl Drop for VimRpcClient {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_client() -> (VimRpcClient, tempfile::TempDir) {
        let tmp_dir = tempdir().unwrap();
        let socket_path = tmp_dir
            .path()
            .join("nvim_test.sock")
            .to_string_lossy()
            .to_string();
        (VimRpcClient::new(socket_path), tmp_dir)
    }

    #[test]
    fn test_vim_rpc_client_creation() {
        let (client, _tmp_dir) = create_test_client();
        assert!(client.nvim_process_id.is_none());
    }

    #[test]
    fn test_start_and_stop_neovim() -> Result<()> {
        let (mut client, tmp_dir) = create_test_client();

        // テストファイルを作成
        let test_file = tmp_dir.path().join("test.txt");
        std::fs::write(&test_file, "hello world\nsecond line")?;

        // Neovimを起動
        client.start_neovim(test_file.to_str().unwrap(), None)?;
        assert!(client.nvim_process_id.is_some());

        // 少し待ってから状態を確認
        std::thread::sleep(std::time::Duration::from_millis(500));

        // 基本的な式評価をテスト
        let mode = client.eval_expr("mode()")?;
        assert_eq!(mode, "n");

        // 停止
        client.stop()?;
        assert!(client.nvim_process_id.is_none());

        Ok(())
    }

    #[test]
    fn test_send_keys_and_get_state() -> Result<()> {
        let (mut client, tmp_dir) = create_test_client();

        let test_file = tmp_dir.path().join("test.txt");
        std::fs::write(&test_file, "hello world")?;

        client.start_neovim(test_file.to_str().unwrap(), None)?;
        std::thread::sleep(std::time::Duration::from_millis(500));

        // 初期状態を取得
        let initial_state = client.get_current_state()?;
        assert_eq!(initial_state.mode, VimMode::Normal);
        assert_eq!(initial_state.cursor_line, 0); // 0ベース
        assert_eq!(initial_state.cursor_col, 0);

        // キーを送信してカーソルを移動
        client.send_keys("ll")?; // 右に2文字移動
        std::thread::sleep(std::time::Duration::from_millis(100));

        let moved_state = client.get_current_state()?;
        assert_eq!(moved_state.cursor_col, 2); // 0ベース

        // Insertモードに入る
        client.send_keys("i")?;
        std::thread::sleep(std::time::Duration::from_millis(100));

        let insert_state = client.get_current_state()?;
        assert_eq!(insert_state.mode, VimMode::Insert);

        client.stop()?;
        Ok(())
    }

    #[test]
    fn test_operator_pending_detection() -> Result<()> {
        let (mut client, tmp_dir) = create_test_client();

        let test_file = tmp_dir.path().join("test.txt");
        std::fs::write(&test_file, "hello world test")?;

        client.start_neovim(test_file.to_str().unwrap(), None)?;
        std::thread::sleep(std::time::Duration::from_millis(500));

        // 'd'キーを押してオペレーター待機モードに入る
        client.send_keys("d")?;
        std::thread::sleep(std::time::Duration::from_millis(200));

        let op_pending_state = client.get_current_state()?;
        assert_eq!(
            op_pending_state.mode,
            VimMode::OperatorPending("d".to_string())
        );

        // Escでキャンセル
        client.send_keys("<Esc>")?;
        std::thread::sleep(std::time::Duration::from_millis(100));

        let normal_state = client.get_current_state()?;
        assert_eq!(normal_state.mode, VimMode::Normal);

        client.stop()?;
        Ok(())
    }

    #[test]
    fn test_yank_and_register_detection() -> Result<()> {
        let (mut client, tmp_dir) = create_test_client();

        let test_file = tmp_dir.path().join("test.txt");
        std::fs::write(&test_file, "hello world")?;

        client.start_neovim(test_file.to_str().unwrap(), None)?;
        std::thread::sleep(std::time::Duration::from_millis(500));

        // 単語をyank
        client.send_keys("yiw")?; // yank inner word
        std::thread::sleep(std::time::Duration::from_millis(200));

        let state_after_yank = client.get_current_state()?;

        // レジスタ0または無名レジスタに"hello"が格納されているはず
        let has_yanked_content = state_after_yank
            .registers
            .get("0")
            .map(|content| content.contains("hello"))
            .unwrap_or(false)
            || state_after_yank
                .registers
                .get("\"")
                .map(|content| content.contains("hello"))
                .unwrap_or(false);

        assert!(
            has_yanked_content,
            "Expected 'hello' to be yanked into registers"
        );

        client.stop()?;
        Ok(())
    }
}
