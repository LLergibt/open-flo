#!/usr/bin/env bash
# Optional project-local build tools; explicit environment settings take priority.
if [ -d "$project_root/.tooling/cargo" ]; then
  export CARGO_HOME="${CARGO_HOME:-$project_root/.tooling/cargo}"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$project_root/.tooling/target}"
fi
if [ -d "$project_root/.tooling/gradle" ]; then
  export GRADLE_USER_HOME="${GRADLE_USER_HOME:-$project_root/.tooling/gradle}"
fi
if [ -d "$project_root/.tooling/android-sdk" ]; then
  export ANDROID_HOME="${ANDROID_HOME:-$project_root/.tooling/android-sdk}"
fi
if [ -d "$project_root/.tooling/npm-cache" ]; then
  export npm_config_cache="${npm_config_cache:-$project_root/.tooling/npm-cache}"
fi
if [ -z "${JAVA_HOME:-}" ]; then
  for local_jdk in "$project_root"/.tooling/jdk-17*/Contents/Home; do
    if [ -d "$local_jdk" ]; then
      export JAVA_HOME="$local_jdk"
      break
    fi
  done
fi
