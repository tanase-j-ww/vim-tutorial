use anyhow::Result;
use clap::Parser;
use std::fs;
use std::process::Command;
use tempfile::NamedTempFile;

mod content;
mod continuous_content;
mod continuous_session;
mod game;
mod vim_rpc;
mod vim_state;

use continuous_content::ContinuousContentLoader;
use continuous_session::{ContinuousVimSession, ExerciseResult};
use game::VimTutorialGame;
use std::io::{self, Write};

#[derive(Parser)]
#[command(name = "vim-tutorial-nvim")]
#[command(about = "Neovimを使ったVimチュートリアルゲーム")]
struct Args {
    #[arg(short, long, help = "テストモードを実行")]
    test: bool,

    #[arg(short, long, help = "連続学習モードを使用")]
    continuous: bool,

    #[arg(long, help = "サンプル章を生成")]
    generate_sample: Option<String>,
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

    if let Some(output_path) = args.generate_sample {
        // サンプル章を生成
        let loader = ContinuousContentLoader::empty();
        loader.create_sample_chapter(&output_path)?;
        println!("✓ サンプル章を生成しました: {}", output_path);
    } else if args.test {
        // テストモード
        test_neovim_integration()?;
    } else if args.continuous {
        // 連続学習モード
        run_continuous_mode()?;
    } else {
        // 従来のゲームモード
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
        println!(
            "Neovim バージョン: {}",
            version.lines().next().unwrap_or("不明")
        );
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
    let vim_script = format!(
        r#"
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
"#,
        sample_file.path().display()
    );

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

fn run_continuous_mode() -> Result<()> {
    println!("=== 🚀 連続学習モード ===\n");

    // コンテンツローダーを初期化
    let content_loader = match ContinuousContentLoader::new() {
        Ok(loader) => loader,
        Err(_) => {
            println!("📝 連続学習用のコンテンツが見つかりません。");
            println!("サンプル章を生成しますか？ [y/N]: ");

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            if input.trim().to_lowercase() == "y" {
                let sample_path = "data/chapters/continuous_chapter_01.yaml";
                std::fs::create_dir_all("data/chapters")?;

                let empty_loader = ContinuousContentLoader::empty();
                empty_loader.create_sample_chapter(sample_path)?;

                println!("\n✓ サンプル章を生成しました: {}", sample_path);
                println!("プログラムを再起動してください。");
                return Ok(());
            } else {
                println!("連続学習モードを終了します。");
                return Ok(());
            }
        }
    };

    // 章選択メニュー
    loop {
        content_loader.list_chapters();

        println!(
            "章番号を選択してください (1-{}, q=終了):",
            content_loader.get_chapter_count()
        );
        print!("選択: ");
        io::stdout().flush()?;

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => {
                println!("デモモード: 第1章を自動選択します。");
                let _ = start_continuous_chapter(&content_loader, 1);
                // 章完了後、メニューに戻る
                continue;
            }
            Ok(_) => {
                let input = input.trim();

                if input == "q" || input == "quit" {
                    println!("連続学習モードを終了します。");
                    break;
                }

                if let Ok(chapter_num) = input.parse::<u8>() {
                    if chapter_num >= 1 && chapter_num <= content_loader.get_chapter_count() as u8 {
                        let _ = start_continuous_chapter(&content_loader, chapter_num);
                        // 章完了後、メニューに戻る
                        continue;
                    } else {
                        println!(
                            "❌ 無効な章番号です。1-{} の範囲で入力してください。",
                            content_loader.get_chapter_count()
                        );
                    }
                } else {
                    println!("❌ 数字または 'q' を入力してください。");
                }
            }
            Err(_) => {
                println!("デモモード: 第1章を自動選択します。");
                let _ = start_continuous_chapter(&content_loader, 1);
                // 章完了後、メニューに戻る
                continue;
            }
        }
    }

    Ok(())
}

fn start_continuous_chapter(
    content_loader: &ContinuousContentLoader,
    chapter_num: u8,
) -> Result<()> {
    if let Some(chapter) = content_loader.get_chapter(chapter_num) {
        println!(
            "\n🎯 === 第{}章: {} ===",
            chapter.chapter.number, chapter.chapter.title
        );
        println!("{}\n", chapter.chapter.description);

        // 一意なソケットパスを生成
        let socket_path = format!("/tmp/vim_tutorial_continuous_{}.sock", std::process::id());
        let mut session = ContinuousVimSession::new(socket_path);

        // 各練習を実行
        for (exercise_index, exercise) in chapter.continuous_exercises.iter().enumerate() {
            println!(
                "📚 === 練習 {}/{}: {} ===",
                exercise_index + 1,
                chapter.continuous_exercises.len(),
                exercise.title
            );

            // サンプルファイルを作成
            let sample_content = exercise.sample_code.join("\n");
            let sample_file = NamedTempFile::new()?;
            fs::write(&sample_file, sample_content)?;

            // 練習を開始
            session.start_exercise(exercise.clone(), sample_file.path().to_str().unwrap())?;

            // 進行を監視
            match session.monitor_progress()? {
                ExerciseResult::Completed => {
                    // 個別タスク完了時は即座に次へ（メッセージなし）
                    if exercise_index < chapter.continuous_exercises.len() - 1 {
                        // tmuxセッションをデタッチして次の練習の準備
                        let _ = std::process::Command::new("tmux")
                            .args(["detach-client", "-s", "vim_tutorial_continuous"])
                            .output();
                        
                        // セッションを停止（次の練習のため）
                        session.stop_exercise()?;
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    } else {
                        // 最後の練習完了 = 章完了
                        session.stop_exercise()?;
                        println!("🎉 第{}章「{}」を完了しました！", chapter.chapter.number, chapter.chapter.title);
                        println!("お疲れ様でした！");
                        break; // 練習ループを抜けてメニューに戻る
                    }
                }
                ExerciseResult::Incomplete => {
                    println!("⏸️ 練習が未完了です。セッションを終了します。");
                    session.stop_exercise()?;
                    break;
                }
                ExerciseResult::Failed(error) => {
                    println!("❌ 練習でエラーが発生しました: {}", error);
                    session.stop_exercise()?;
                    break;
                }
            }
        }
    } else {
        println!("❌ 第{}章が見つかりません。", chapter_num);
    }

    Ok(())
}
