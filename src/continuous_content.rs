use crate::continuous_session::ContinuousExercise;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContinuousChapterData {
    pub chapter: ChapterInfo,
    pub continuous_exercises: Vec<ContinuousExercise>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChapterInfo {
    pub number: u8,
    pub title: String,
    pub description: String,
}

pub struct ContinuousContentLoader {
    chapters: Vec<ContinuousChapterData>,
}

impl ContinuousContentLoader {
    pub fn empty() -> Self {
        Self { chapters: vec![] }
    }

    pub fn new() -> Result<Self> {
        let mut chapters = Vec::new();

        // 連続学習用の章ファイルを読み込み
        for chapter_num in 1..=8 {
            let file_path = format!("data/chapters/continuous_chapter_{:02}.yaml", chapter_num);

            if Path::new(&file_path).exists() {
                match Self::load_chapter_file(&file_path) {
                    Ok(chapter) => {
                        println!(
                            "✓ 第{}章（連続学習版）を読み込みました: {}",
                            chapter_num, chapter.chapter.title
                        );
                        chapters.push(chapter);
                    }
                    Err(e) => {
                        eprintln!(
                            "⚠️ 第{}章（連続学習版）の読み込みに失敗: {}",
                            chapter_num, e
                        );
                    }
                }
            } else {
                // 従来形式からの自動変換を試みる
                let legacy_file_path = format!("data/chapters/chapter_{:02}.yaml", chapter_num);
                if Path::new(&legacy_file_path).exists() {
                    println!(
                        "🔄 第{}章を従来形式から連続学習形式に変換中...",
                        chapter_num
                    );
                    // TODO: 実装する場合はここで変換処理を行う
                }
            }
        }

        if chapters.is_empty() {
            return Err(anyhow::anyhow!("連続学習コンテンツが見つかりませんでした"));
        }

        println!(
            "📚 合計 {} 章の連続学習コンテンツを読み込みました",
            chapters.len()
        );

        Ok(Self { chapters })
    }

    fn load_chapter_file(file_path: &str) -> Result<ContinuousChapterData> {
        let content = fs::read_to_string(file_path)?;
        let chapter: ContinuousChapterData = serde_yaml::from_str(&content)?;
        Ok(chapter)
    }

    pub fn get_chapter(&self, chapter_num: u8) -> Option<&ContinuousChapterData> {
        self.chapters
            .iter()
            .find(|ch| ch.chapter.number == chapter_num)
    }

    pub fn get_chapter_count(&self) -> usize {
        self.chapters.len()
    }

    pub fn list_chapters(&self) {
        println!("\n=== 利用可能な章（連続学習版） ===");
        for chapter in &self.chapters {
            println!("第{}章: {}", chapter.chapter.number, chapter.chapter.title);
            println!("  {}", chapter.chapter.description);
            println!("  連続練習問題数: {}", chapter.continuous_exercises.len());

            // 各練習の概要を表示
            for (i, exercise) in chapter.continuous_exercises.iter().enumerate() {
                println!(
                    "    {}. {} (目標数: {})",
                    i + 1,
                    exercise.title,
                    exercise.goals.len()
                );
            }
            println!();
        }
    }

    // デバッグ用：サンプル章を生成
    pub fn create_sample_chapter(&self, output_path: &str) -> Result<()> {
        let sample_chapter = ContinuousChapterData {
            chapter: ChapterInfo {
                number: 1,
                title: "基本移動とモード切替".to_string(),
                description: "Vimの基本的なカーソル移動とモード切替を連続して学習します"
                    .to_string(),
            },
            continuous_exercises: vec![
                ContinuousExercise {
                    title: "hjkl移動マスター".to_string(),
                    description: "hjklキーを使って効率的にカーソルを移動します".to_string(),
                    sample_code: vec![
                        "let x = 10;".to_string(),
                        "let y = 20;".to_string(),
                        "let z = 30;".to_string(),
                    ],
                    goals: vec![
                        crate::continuous_session::ExerciseGoal {
                            goal_type: "position".to_string(),
                            target: serde_json::json!([0, 3]),
                            description: "右に3文字移動してください（lll）".to_string(),
                            hint: Some("l キーを3回押します".to_string()),
                        },
                        crate::continuous_session::ExerciseGoal {
                            goal_type: "position".to_string(),
                            target: serde_json::json!([1, 3]),
                            description: "下の行の同じ位置に移動してください（j）".to_string(),
                            hint: Some("j キーで下に移動します".to_string()),
                        },
                        crate::continuous_session::ExerciseGoal {
                            goal_type: "position".to_string(),
                            target: serde_json::json!([1, 0]),
                            description: "行の最初に戻ってください（hhh）".to_string(),
                            hint: Some("h キーで左に移動します".to_string()),
                        },
                        crate::continuous_session::ExerciseGoal {
                            goal_type: "position".to_string(),
                            target: serde_json::json!([0, 0]),
                            description: "最初の行に戻ってください（k）".to_string(),
                            hint: Some("k キーで上に移動します".to_string()),
                        },
                    ],
                    flow_type: crate::continuous_session::FlowType::Sequential,
                },
                ContinuousExercise {
                    title: "モード切替とテキスト入力".to_string(),
                    description: "InsertモードとNormalモードを切り替えながらテキストを編集します"
                        .to_string(),
                    sample_code: vec![
                        "function greet(name) {".to_string(),
                        "  console.log('Hello, ');".to_string(),
                        "}".to_string(),
                    ],
                    goals: vec![
                        crate::continuous_session::ExerciseGoal {
                            goal_type: "position".to_string(),
                            target: serde_json::json!([1, 20]),
                            description: "2行目の'Hello, 'の後に移動してください".to_string(),
                            hint: Some("jで下に移動し、lで右に移動します".to_string()),
                        },
                        crate::continuous_session::ExerciseGoal {
                            goal_type: "mode".to_string(),
                            target: serde_json::json!("insert"),
                            description: "Insertモードに入ってください（i）".to_string(),
                            hint: Some("i キーでInsertモードに入ります".to_string()),
                        },
                        crate::continuous_session::ExerciseGoal {
                            goal_type: "text".to_string(),
                            target: serde_json::json!({
                                "line": 1,
                                "expected": "  console.log('Hello, ' + name);"
                            }),
                            description: "' + name'を入力してください".to_string(),
                            hint: Some("通常通りタイピングします".to_string()),
                        },
                        crate::continuous_session::ExerciseGoal {
                            goal_type: "mode".to_string(),
                            target: serde_json::json!("normal"),
                            description: "Escキーでノーマルモードに戻ってください".to_string(),
                            hint: Some("Esc キーでモードを切り替えます".to_string()),
                        },
                    ],
                    flow_type: crate::continuous_session::FlowType::Sequential,
                },
                ContinuousExercise {
                    title: "削除とヤンク操作".to_string(),
                    description: "deleteとyank操作を組み合わせて効率的に編集します".to_string(),
                    sample_code: vec![
                        "const old_name = 'Alice';".to_string(),
                        "const new_name = 'Bob';".to_string(),
                    ],
                    goals: vec![
                        crate::continuous_session::ExerciseGoal {
                            goal_type: "position".to_string(),
                            target: serde_json::json!([0, 13]),
                            description: "1行目の'Alice'の位置に移動してください".to_string(),
                            hint: None,
                        },
                        crate::continuous_session::ExerciseGoal {
                            goal_type: "mode".to_string(),
                            target: serde_json::json!("operator_d"),
                            description: "削除操作を開始してください（d）".to_string(),
                            hint: Some(
                                "d キーを押してoperator-pendingモードに入ります".to_string(),
                            ),
                        },
                        crate::continuous_session::ExerciseGoal {
                            goal_type: "register".to_string(),
                            target: serde_json::json!({
                                "register": "0",
                                "expected": "Alice"
                            }),
                            description: "単語を削除してヤンクしてください（diw）".to_string(),
                            hint: Some("iw で inner word を指定します".to_string()),
                        },
                        crate::continuous_session::ExerciseGoal {
                            goal_type: "position".to_string(),
                            target: serde_json::json!([1, 13]),
                            description: "2行目の'Bob'の位置に移動してください".to_string(),
                            hint: None,
                        },
                        crate::continuous_session::ExerciseGoal {
                            goal_type: "text".to_string(),
                            target: serde_json::json!({
                                "line": 1,
                                "expected": "const new_name = 'Alice';"
                            }),
                            description: "'Bob'を削除して'Alice'をペーストしてください（ciwp）"
                                .to_string(),
                            hint: Some("ciw で単語を変更、p でペーストします".to_string()),
                        },
                    ],
                    flow_type: crate::continuous_session::FlowType::Sequential,
                },
            ],
        };

        let yaml_content = serde_yaml::to_string(&sample_chapter)?;
        fs::write(output_path, yaml_content)?;
        println!("📝 サンプル章を作成しました: {}", output_path);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sample_chapter_creation() -> Result<()> {
        let tmp_dir = tempdir()?;
        let output_path = tmp_dir.path().join("test_chapter.yaml");

        let loader = ContinuousContentLoader { chapters: vec![] };
        loader.create_sample_chapter(output_path.to_str().unwrap())?;

        // 作成されたファイルを読み込んで確認
        assert!(output_path.exists());
        let content = fs::read_to_string(&output_path)?;
        assert!(content.contains("基本移動とモード切替"));
        assert!(content.contains("hjkl移動マスター"));

        // YAMLとしてパースできるか確認
        let parsed: ContinuousChapterData = serde_yaml::from_str(&content)?;
        assert_eq!(parsed.chapter.number, 1);
        assert_eq!(parsed.continuous_exercises.len(), 3);

        Ok(())
    }

    #[test]
    fn test_continuous_content_loader_structure() {
        let loader = ContinuousContentLoader { chapters: vec![] };
        assert_eq!(loader.get_chapter_count(), 0);
        assert!(loader.get_chapter(1).is_none());
    }
}
