use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChapterData {
    pub chapter: ChapterInfo,
    pub exercises: Vec<ExerciseData>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChapterInfo {
    pub number: u8,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExerciseData {
    pub title: String,
    pub description: String,
    pub sample_code: Vec<String>,
    pub steps: Vec<StepData>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StepData {
    pub instruction: String,
    pub explanation: String,
    pub expected_input: String,
    pub cursor_start: Option<[usize; 2]>,
    pub cursor_end: Option<[usize; 2]>,
    pub mode_change: Option<String>,
    pub text_change: Option<bool>,
}

pub struct ContentLoader {
    chapters: Vec<ChapterData>,
}

impl ContentLoader {
    pub fn new() -> Result<Self> {
        let mut chapters = Vec::new();
        
        // 各章のYAMLファイルを読み込み
        for chapter_num in 1..=8 {
            let file_path = format!("data/chapters/chapter_{:02}.yaml", chapter_num);
            
            if Path::new(&file_path).exists() {
                match Self::load_chapter_file(&file_path) {
                    Ok(chapter) => {
                        println!("✓ 第{}章を読み込みました: {}", chapter_num, chapter.chapter.title);
                        chapters.push(chapter);
                    }
                    Err(e) => {
                        eprintln!("⚠️ 第{}章の読み込みに失敗: {}", chapter_num, e);
                    }
                }
            } else {
                println!("⚠️ ファイルが見つかりません: {}", file_path);
            }
        }
        
        if chapters.is_empty() {
            return Err(anyhow::anyhow!("学習コンテンツが見つかりませんでした"));
        }
        
        println!("📚 合計 {} 章の学習コンテンツを読み込みました", chapters.len());
        
        Ok(Self { chapters })
    }
    
    fn load_chapter_file(file_path: &str) -> Result<ChapterData> {
        let content = fs::read_to_string(file_path)?;
        let chapter: ChapterData = serde_yaml::from_str(&content)?;
        Ok(chapter)
    }
    
    pub fn get_chapter(&self, chapter_num: u8) -> Option<&ChapterData> {
        self.chapters.iter().find(|ch| ch.chapter.number == chapter_num)
    }
    
    
    pub fn get_chapter_count(&self) -> usize {
        self.chapters.len()
    }
    
    pub fn list_chapters(&self) {
        println!("\n=== 利用可能な章 ===");
        for chapter in &self.chapters {
            println!("第{}章: {}", chapter.chapter.number, chapter.chapter.title);
            println!("  {}", chapter.chapter.description);
            println!("  練習問題数: {}", chapter.exercises.len());
            println!();
        }
    }
}