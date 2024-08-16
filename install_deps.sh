wget -qO- https://get.pnpm.io/install.sh | sh -
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash

export NVM_DIR="$([ -z "${XDG_CONFIG_HOME-}" ] && printf %s "${HOME}/.nvm" || printf %s "${XDG_CONFIG_HOME}/nvm")"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"  # This loads nvm
nvm install 20

export PATH="$PATH:$HOME/.local/share/pnpm"
export PNPM_HOME="$HOME/.local/share/pnpm"
export PATH="$PNPM_HOME:$PATH"

# Verify pnpm installation
if command -v pnpm >/dev/null; then
    echo "pnpm has been successfully installed and is available."
else
    echo "There was a problem adding pnpm to the PATH. Please check the installation."
    exit 1  # Exit if pnpm isn't available
fi

cd frontend
pnpm install
pnpm build:production