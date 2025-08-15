#!/bin/bash

# Vim Tutorial Game (Neovim版) インストールスクリプト
# Usage: bash install.sh

set -e

echo "🚀 Vim Tutorial Game (Neovim版) セットアップを開始します..."
echo

# 色付き出力用の関数
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

success() {
    echo -e "${GREEN}✓ $1${NC}"
}

error() {
    echo -e "${RED}✗ $1${NC}"
}

warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

info() {
    echo -e "ℹ $1"
}

# システム情報の確認
info "システム情報を確認中..."
OS=$(uname -s)
case "$OS" in
    Linux*)
        PLATFORM="Linux"
        PKG_MANAGER=""
        if command -v apt >/dev/null 2>&1; then
            PKG_MANAGER="apt"
        elif command -v dnf >/dev/null 2>&1; then
            PKG_MANAGER="dnf"
        elif command -v yum >/dev/null 2>&1; then
            PKG_MANAGER="yum"
        elif command -v pacman >/dev/null 2>&1; then
            PKG_MANAGER="pacman"
        fi
        ;;
    Darwin*)
        PLATFORM="macOS"
        PKG_MANAGER="brew"
        ;;
    *)
        PLATFORM="Unknown"
        ;;
esac

info "検出されたプラットフォーム: $PLATFORM"
echo

# Rust の確認とインストール
info "Rust の確認中..."
if command -v cargo >/dev/null 2>&1; then
    RUST_VERSION=$(rustc --version | cut -d' ' -f2)
    success "Rust が見つかりました (バージョン: $RUST_VERSION)"
else
    warning "Rust が見つかりません。インストールします..."
    
    if command -v curl >/dev/null 2>&1; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source ~/.cargo/env
        success "Rust をインストールしました"
    else
        error "curl が見つかりません。手動でRustをインストールしてください:"
        error "https://rustup.rs/ を参照"
        exit 1
    fi
fi

# Neovim の確認とインストール
info "Neovim の確認中..."
if command -v nvim >/dev/null 2>&1; then
    NVIM_VERSION=$(nvim --version | head -n1 | cut -d' ' -f2)
    success "Neovim が見つかりました (バージョン: $NVIM_VERSION)"
else
    warning "Neovim が見つかりません。インストールを試行します..."
    
    case "$PLATFORM" in
        Linux)
            case "$PKG_MANAGER" in
                apt)
                    info "apt を使用してNeovimをインストール中..."
                    sudo apt update && sudo apt install -y neovim
                    ;;
                dnf)
                    info "dnf を使用してNeovimをインストール中..."
                    sudo dnf install -y neovim
                    ;;
                yum)
                    info "yum を使用してNeovimをインストール中..."
                    sudo yum install -y neovim
                    ;;
                pacman)
                    info "pacman を使用してNeovimをインストール中..."
                    sudo pacman -S neovim
                    ;;
                *)
                    error "未対応のパッケージマネージャーです。手動でNeovimをインストールしてください。"
                    exit 1
                    ;;
            esac
            ;;
        macOS)
            if command -v brew >/dev/null 2>&1; then
                info "Homebrew を使用してNeovimをインストール中..."
                brew install neovim
            else
                error "Homebrew が見つかりません。手動でNeovimをインストールしてください。"
                exit 1
            fi
            ;;
        *)
            error "未対応のプラットフォームです。手動でNeovimをインストールしてください。"
            exit 1
            ;;
    esac
    
    # インストール確認
    if command -v nvim >/dev/null 2>&1; then
        success "Neovim をインストールしました"
    else
        error "Neovim のインストールに失敗しました"
        exit 1
    fi
fi

echo

# プロジェクトのビルド
info "プロジェクトをビルド中..."
if cargo build --release; then
    success "ビルドが完了しました"
else
    error "ビルドに失敗しました"
    exit 1
fi

echo

# 動作確認テスト
info "動作確認テストを実行中..."
if cargo run --release -- --test >/dev/null 2>&1; then
    success "動作確認テストが成功しました"
else
    warning "動作確認テストに問題がありました（実行は可能です）"
fi

echo

# セットアップ完了
success "🎉 セットアップが完了しました！"
echo
echo "=== 実行方法 ==="
echo
echo "1. 基本実行:"
echo "   cargo run --release"
echo
echo "2. テストモード:"
echo "   cargo run --release -- --test"
echo
echo "3. ヘルプ表示:"
echo "   cargo run --release -- --help"
echo
echo "=== ファイル構成 ==="
echo
echo "📁 プロジェクトディレクトリ: $(pwd)"
echo "📄 README.md: 詳細な使用方法"
echo "📄 DEMO.md: 実行例とデモ"
echo "📂 data/chapters/: 学習コンテンツ (YAML)"
echo "🎯 target/release/vim-tutorial-nvim: 実行可能ファイル"
echo
echo "楽しいVim学習を始めましょう！ 🚀"
echo
echo "問題が発生した場合は、README.md のトラブルシューティングセクションを参照してください。"