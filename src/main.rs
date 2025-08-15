use anyhow::Result;
use std::process::Command;
use std::fs;
use tempfile::NamedTempFile;
use clap::Parser;

mod game;
mod content;
use game::VimTutorialGame;

#[derive(Parser)]
#[command(name = "vim-tutorial-nvim")]
#[command(about = "Neovimを使ったVimチュートリアルゲーム")]
struct Args {
    #[arg(short, long, help = "テストモードを実行")]
    test: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    // Neovimが利用可能かチェック
    match check_neovim_available() {
        Ok(_) => println!("✓ Neovim が見つかりました"),
        Err(e) => {
            eprintln!("✗ Neovim が見つかりません: {}", e);
            eprintln!("  インストール方法: sudo apt install neovim  または  brew install neovim");
            return Err(e);
        }
    }
    
    if args.test {
        // テストモード
        test_neovim_integration()?;
    } else {
        // ゲームモード
        let mut game = VimTutorialGame::new()?;
        game.run()?;
    }
    
    Ok(())
}

fn check_neovim_available() -> Result<()> {
    let output = Command::new("nvim")
        .arg("--version")
        .output()
        .map_err(|_| anyhow::anyhow!("Neovim が見つかりません"))?;
        
    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout);
        println!("Neovim バージョン: {}", version.lines().next().unwrap_or("不明"));
        Ok(())
    } else {
        Err(anyhow::anyhow!("Neovim の実行に失敗しました"))
    }
}

fn test_neovim_integration() -> Result<()> {
    println!("\n=== Neovim連携テスト（Vimスクリプトアプローチ） ===");
    
    // サンプルテキストファイルを作成
    let sample_content = r#"function example() {
  let name = 'world';
  console.log('Hello, ' + name);
}"#;
    
    let sample_file = NamedTempFile::new()?;
    fs::write(&sample_file, sample_content)?;
    
    println!("✓ サンプルファイルを作成しました: {:?}", sample_file.path());
    
    // Vimスクリプトを作成してキー入力をテスト
    let vim_script = format!(r#"
" ファイルを開く
edit {}

" 初期位置に移動 (1行目, 1列目)
normal! gg0

" 現在のカーソル位置を出力
let initial_pos = [line('.'), col('.')]
call writefile(['INITIAL:' . initial_pos[0] . ',' . initial_pos[1]], '/tmp/vim_test_output.txt')

" キー入力をシミュレート: jjl (下下右)
normal! jjl

" 新しいカーソル位置を出力
let final_pos = [line('.'), col('.')]
call writefile(['FINAL:' . final_pos[0] . ',' . final_pos[1]], '/tmp/vim_test_output.txt', 'a')

" 期待される位置と比較 (3行目, 2列目)
if final_pos == [3, 2]
    call writefile(['RESULT:SUCCESS'], '/tmp/vim_test_output.txt', 'a')
else
    call writefile(['RESULT:FAILED'], '/tmp/vim_test_output.txt', 'a')
endif

" 終了
qa!
"#, sample_file.path().display());
    
    let script_file = NamedTempFile::new()?;
    fs::write(&script_file, vim_script)?;
    
    println!("✓ Vimスクリプトを作成しました");
    
    // Neovimでスクリプトを実行
    let output = Command::new("nvim")
        .arg("--headless")
        .arg("-S")
        .arg(script_file.path())
        .output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Neovim実行エラー: {}", stderr));
    }
    
    println!("✓ Neovimスクリプトを実行しました");
    
    // 結果を読み取り
    if let Ok(result_content) = fs::read_to_string("/tmp/vim_test_output.txt") {
        println!("=== テスト結果 ===");
        for line in result_content.lines() {
            if let Some(pos) = line.strip_prefix("INITIAL:") {
                println!("初期カーソル位置: {}", pos);
            } else if let Some(pos) = line.strip_prefix("FINAL:") {
                println!("最終カーソル位置: {}", pos);
            } else if let Some(result) = line.strip_prefix("RESULT:") {
                if result == "SUCCESS" {
                    println!("✓ キー入力の正解判定: 成功");
                } else {
                    println!("✗ キー入力の正解判定: 失敗");
                }
            }
        }
        
        // 一時ファイルをクリーンアップ
        let _ = fs::remove_file("/tmp/vim_test_output.txt");
    } else {
        return Err(anyhow::anyhow!("テスト結果ファイルの読み取りに失敗"));
    }
    
    println!("✓ Neovim連携テスト完了");
    
    Ok(())
}
