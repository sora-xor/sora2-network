#!/bin/sh

set -eu

normalize_libclang_path() {
  path=$1
  if [ -z "${path}" ]; then
    return 1
  fi

  if [ -d "${path}" ] && has_libclang "${path}"; then
    printf '%s\n' "${path}"
    return 0
  fi

  if [ -f "${path}" ]; then
    case "$(basename -- "${path}")" in
      libclang.dylib|libclang.so|libclang.so.*)
        dirname -- "${path}"
        return 0
        ;;
    esac
  fi

  return 1
}

has_libclang() {
  dir=$1
  [ -f "$dir/libclang.dylib" ] || [ -f "$dir/libclang.so" ] || ls "$dir"/libclang.so.* >/dev/null 2>&1
}

prepend_path() {
  value=$1
  current=${2-}
  if [ -n "$current" ]; then
    printf '%s:%s' "$value" "$current"
  else
    printf '%s' "$value"
  fi
}

detect_llvm_libdir() {
  if normalized_libclang_path=$(normalize_libclang_path "${LIBCLANG_PATH:-}" 2>/dev/null); then
    printf '%s\n' "${normalized_libclang_path}"
    return 0
  fi

  if [ -n "${LLVM_CONFIG_PATH:-}" ] && [ -x "${LLVM_CONFIG_PATH}" ]; then
    "${LLVM_CONFIG_PATH}" --libdir
    return 0
  fi

  if command -v llvm-config >/dev/null 2>&1; then
    llvm-config --libdir
    return 0
  fi

  if command -v brew >/dev/null 2>&1; then
    brew_prefix=$(brew --prefix llvm 2>/dev/null || true)
    if [ -n "${brew_prefix}" ] && has_libclang "${brew_prefix}/lib"; then
      printf '%s\n' "${brew_prefix}/lib"
      return 0
    fi
  fi

  for libdir in \
    /opt/homebrew/opt/llvm/lib \
    /usr/local/opt/llvm/lib \
    /usr/lib/llvm-18/lib \
    /usr/lib/llvm-17/lib \
    /usr/lib/llvm-16/lib \
    /usr/lib/llvm-15/lib \
    /usr/lib/llvm-14/lib
  do
    if has_libclang "${libdir}"; then
      printf '%s\n' "${libdir}"
      return 0
    fi
  done

  return 1
}

if llvm_libdir=$(detect_llvm_libdir); then
  export LIBCLANG_PATH="${llvm_libdir}"

  if [ -z "${LLVM_CONFIG_PATH:-}" ] && command -v brew >/dev/null 2>&1; then
    brew_prefix=$(brew --prefix llvm 2>/dev/null || true)
    if [ -n "${brew_prefix}" ] && [ -x "${brew_prefix}/bin/llvm-config" ]; then
      export LLVM_CONFIG_PATH="${brew_prefix}/bin/llvm-config"
    fi
  fi

  case "$(uname -s)" in
    Darwin)
      export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
      export DYLD_FALLBACK_LIBRARY_PATH="$(prepend_path "${llvm_libdir}" "${DYLD_FALLBACK_LIBRARY_PATH:-}")"
      ;;
    *)
      export LD_LIBRARY_PATH="$(prepend_path "${llvm_libdir}" "${LD_LIBRARY_PATH:-}")"
      ;;
  esac
fi

exec "$@"
