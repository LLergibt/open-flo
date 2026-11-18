#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$project_root/scripts/tooling.sh"
cd "$project_root"

cargo fmt --all -- --check
cargo test --locked --workspace
npm --prefix apps/web run check
npm --prefix apps/web test
npm --prefix apps/web run build

if [ "${CHECK_ANDROID:-0}" = "1" ]; then
  "$project_root/scripts/android.sh" assembleDebug lintDebug testDebugUnitTest
else
  echo "Android build not requested. With Android SDK/JDK ready: CHECK_ANDROID=1 ./scripts/check.sh"
fi
