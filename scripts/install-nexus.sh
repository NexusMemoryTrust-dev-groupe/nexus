#!/bin/bash

# ================================================================
#  Nexus Memory Trust - Linux Installer
# ================================================================

set -e

# Configuration
GITHUB_REPO="NexusMemoryTrust-dev-groupe/nexus"
APP_NAME="Nexus Memory Trust"
INSTALL_DIR="$HOME/.local/share/nexus"
BINARY_NAME="nexus"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Functions
print_header() {
    echo ""
    echo -e "${CYAN}================================================================${NC}"
    echo -e "${CYAN}           Nexus Memory Trust - Linux Installer                ${NC}"
    echo -e "${CYAN}================================================================${NC}"
    echo ""
}

print_step() {
    echo -e "${YELLOW}> ${NC}$1"
}

print_success() {
    echo -e "${GREEN}  [OK] ${NC}$1"
}

print_error() {
    echo -e "${RED}  [FAIL] ${NC}$1"
}

get_latest_version() {
    print_step "Checking latest version..."
    
    VERSION=$(curl -s "https://api.github.com/repos/$GITHUB_REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
    
    if [ -z "$VERSION" ]; then
        print_error "Failed to check latest version"
        exit 1
    fi
    
    print_success "Latest version: $VERSION"
}

download_binary() {
    print_step "Downloading Nexus..."
    
    # Detect architecture
    ARCH=$(uname -m)
    case $ARCH in
        x86_64)
            ASSET_NAME="nexus-linux-x64"
            ;;
        aarch64)
            ASSET_NAME="nexus-linux-arm64"
            ;;
        *)
            print_error "Unsupported architecture: $ARCH"
            exit 1
            ;;
    esac
    
    # Create install directory
    mkdir -p "$INSTALL_DIR"
    
    # Download binary
    DOWNLOAD_URL="https://github.com/$GITHUB_REPO/releases/download/$VERSION/$ASSET_NAME"
    
    if ! curl -L -o "$INSTALL_DIR/$BINARY_NAME" "$DOWNLOAD_URL" 2>/dev/null; then
        print_error "Failed to download Nexus"
        exit 1
    fi
    
    chmod +x "$INSTALL_DIR/$BINARY_NAME"
    
    print_success "Downloaded to: $INSTALL_DIR/$BINARY_NAME"
    
    # Download icon
    print_step "Downloading icon..."
    ICON_DIR="$HOME/.local/share/icons"
    mkdir -p "$ICON_DIR"
    
    ICON_URL="https://github.com/$GITHUB_REPO/releases/download/$VERSION/nexus-icon.png"
    if curl -L -o "$ICON_DIR/nexus.png" "$ICON_URL" 2>/dev/null; then
        print_success "Icon downloaded to: $ICON_DIR/nexus.png"
    else
        print_error "Failed to download icon (app will use default icon)"
    fi
}

add_to_path() {
    print_step "Adding to PATH..."
    
    SHELL_CONFIG=""
    if [ -f "$HOME/.bashrc" ]; then
        SHELL_CONFIG="$HOME/.bashrc"
    elif [ -f "$HOME/.zshrc" ]; then
        SHELL_CONFIG="$HOME/.zshrc"
    fi
    
    if [ -n "$SHELL_CONFIG" ]; then
        if ! grep -q "$INSTALL_DIR" "$SHELL_CONFIG" 2>/dev/null; then
            echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$SHELL_CONFIG"
            export PATH="$PATH:$INSTALL_DIR"
            print_success "Added to $SHELL_CONFIG"
        else
            print_success "Already in PATH"
        fi
    else
        # Fallback to .profile
        if ! grep -q "$INSTALL_DIR" "$HOME/.profile" 2>/dev/null; then
            echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$HOME/.profile"
            export PATH="$PATH:$INSTALL_DIR"
            print_success "Added to ~/.profile"
        else
            print_success "Already in PATH"
        fi
    fi
}

create_desktop_entry() {
    print_step "Creating desktop entry..."
    
    DESKTOP_DIR="$HOME/.local/share/applications"
    mkdir -p "$DESKTOP_DIR"
    
    DESKTOP_FILE="$DESKTOP_DIR/nexus.desktop"
    
    # Use downloaded icon if available, otherwise fallback to generic
    ICON_PATH="$HOME/.local/share/icons/nexus.png"
    if [ ! -f "$ICON_PATH" ]; then
        ICON_PATH="nexus"
    fi
    
    cat > "$DESKTOP_FILE" << EOF
[Desktop Entry]
Name=$APP_NAME
Comment=AI Memory Operating System
Exec=$INSTALL_DIR/$BINARY_NAME
Icon=$ICON_PATH
Terminal=false
Type=Application
Categories=Utility;Productivity;
Keywords=memory;ai;knowledge;
EOF
    
    chmod +x "$DESKTOP_FILE"
    
    print_success "Desktop entry created: $DESKTOP_FILE"
}

create_uninstaller() {
    print_step "Creating uninstaller..."
    
    UNINSTALL_SCRIPT="$INSTALL_DIR/uninstall.sh"
    
    cat > "$UNINSTALL_SCRIPT" << 'EOF'
#!/bin/bash

echo "Uninstalling Nexus Memory Trust..."

# Remove installation directory
rm -rf "$HOME/.local/share/nexus"

# Remove desktop entry
rm -f "$HOME/.local/share/applications/nexus.desktop"

# Remove from PATH
if [ -f "$HOME/.bashrc" ]; then
    sed -i '/export PATH.*\.local\/share\/nexus/d' "$HOME/.bashrc"
fi

if [ -f "$HOME/.zshrc" ]; then
    sed -i '/export PATH.*\.local\/share\/nexus/d' "$HOME/.zshrc"
fi

echo "Nexus Memory Trust has been uninstalled."
EOF
    
    chmod +x "$UNINSTALL_SCRIPT"
    
    print_success "Uninstaller created: $UNINSTALL_SCRIPT"
}

# ═══════════════════════════════════════════════════════════════
#  Main
# ═══════════════════════════════════════════════════════════════

print_header

# Check if already installed
if [ -f "$INSTALL_DIR/$BINARY_NAME" ]; then
    echo -e "${YELLOW}⚠ Nexus is already installed at: $INSTALL_DIR/$BINARY_NAME${NC}"
    read -p "Do you want to reinstall? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Installation cancelled."
        exit 0
    fi
fi

# Check dependencies
print_step "Checking dependencies..."
if ! command -v curl &> /dev/null; then
    print_error "curl is required but not installed"
    print_error "Install with: sudo apt install curl"
    exit 1
fi
print_success "curl found"

# Run installation
get_latest_version
download_binary
add_to_path
create_desktop_entry
create_uninstaller

# Success message
echo ""
echo -e "${GREEN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║                    Installation Complete!                   ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${CYAN}🚀 Launch Nexus:${NC}"
echo "   • Find 'Nexus Memory Trust' in your applications menu"
echo "   • Or run: nexus"
echo ""
echo -e "${CYAN}📚 First launch:${NC}"
echo "   • The app will configure OpenCode CLI for you"
echo "   • Choose your preferred AI model"
echo ""
echo -e "${CYAN}🗑️  Uninstall:${NC}"
echo "   • Run: $INSTALL_DIR/uninstall.sh"
echo ""
