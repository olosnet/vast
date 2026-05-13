#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
    cat <<'USAGE'
Usage: scripts/update-svbony-drivers.sh [--no-check] <sdk-dir|sdk-archive|sdk-url>

Update vendored SVBONY/SVB Camera SDK files under external/svb.

Accepted input:
  - extracted SDK directory
  - .zip, .tar.gz, .tgz, .tar.bz2, .tbz2, .tar.xz, or .txz archive
  - http(s) URL pointing to one of those archives

Expected SDK content:
  - SVBCameraSDK.h
  - libSVBCameraSDK.a, libSVBCameraSDK.so, and libusb-1.0.so for x64, x86, armv8, armv7, armv6

By default, runs cargo check --workspace after update so bindgen refreshes
lvast/src/bindings/svb.rs. Use --no-check to skip that step.
USAGE
}

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

need_command() {
    command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

script_dir=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
repo_root=$(CDPATH= cd -- "${script_dir}/.." && pwd -P)
dest="${repo_root}/external/svb"
run_check=1
source_input=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help)
            usage
            exit 0
            ;;
        --no-check)
            run_check=0
            shift
            ;;
        --source)
            [[ $# -ge 2 ]] || die "--source needs a path or URL"
            source_input=$2
            shift 2
            ;;
        --*)
            die "unknown option: $1"
            ;;
        *)
            [[ -z "$source_input" ]] || die "only one SDK source is supported"
            source_input=$1
            shift
            ;;
    esac
done

[[ -n "$source_input" ]] || { usage >&2; exit 2; }

tmp_dir=$(mktemp -d)
cleanup() {
    rm -rf "$tmp_dir"
}
trap cleanup EXIT

extract_archive() {
    local archive=$1
    local out_dir=$2

    mkdir -p "$out_dir"
    case "$archive" in
        *.tar.gz|*.tgz)
            need_command tar
            tar -xzf "$archive" -C "$out_dir"
            ;;
        *.tar.bz2|*.tbz2)
            need_command tar
            tar -xjf "$archive" -C "$out_dir"
            ;;
        *.tar.xz|*.txz)
            need_command tar
            tar -xJf "$archive" -C "$out_dir"
            ;;
        *.zip)
            need_command unzip
            unzip -q "$archive" -d "$out_dir"
            ;;
        *)
            die "unsupported archive type: $archive"
            ;;
    esac
}

download_source() {
    local url=$1
    local name
    local output

    name=$(basename -- "${url%%\?*}")
    [[ -n "$name" && "$name" != "/" ]] || name="svbony-sdk-download"
    output="${tmp_dir}/${name}"

    if command -v curl >/dev/null 2>&1; then
        curl -fL "$url" -o "$output"
    elif command -v wget >/dev/null 2>&1; then
        wget -O "$output" "$url"
    else
        die "need curl or wget to download URL"
    fi

    printf '%s\n' "$output"
}

prepare_source_root() {
    local input=$1
    local archive
    local extract_dir

    if [[ "$input" =~ ^https?:// ]]; then
        archive=$(download_source "$input")
        extract_dir="${tmp_dir}/extract"
        extract_archive "$archive" "$extract_dir"
        printf '%s\n' "$extract_dir"
    elif [[ -d "$input" ]]; then
        (CDPATH= cd -- "$input" && pwd -P)
    elif [[ -f "$input" ]]; then
        extract_dir="${tmp_dir}/extract"
        extract_archive "$input" "$extract_dir"
        printf '%s\n' "$extract_dir"
    else
        die "SDK source not found: $input"
    fi
}

find_one() {
    local root=$1
    local name=$2
    local match

    match=$(find "$root" \( -type f -o -type l \) -name "$name" -print -quit)
    [[ -n "$match" ]] || die "missing $name under $root"
    printf '%s\n' "$match"
}

find_arch_file() {
    local root=$1
    local arch=$2
    local name=$3
    local match

    match=$(find "$root" \( -type f -o -type l \) -path "*/${arch}/*" -name "$name" -print -quit)
    if [[ -z "$match" ]]; then
        match=$(find "$root" \( -type f -o -type l \) -path "*${arch}*" -name "$name" -print -quit)
    fi
    [[ -n "$match" ]] || die "missing ${name} for ${arch} under $root"
    printf '%s\n' "$match"
}

copy_arch_libs() {
    local source_root=$1
    local arch=$2
    local arch_dir=$3
    local sdk_static
    local sdk_shared
    local sdk_shared_dir
    local usb_shared
    local usb_shared_dir
    local path

    mkdir -p "$arch_dir"

    sdk_static=$(find_arch_file "$source_root" "$arch" libSVBCameraSDK.a)
    cp -a "$sdk_static" "$arch_dir/"

    sdk_shared=$(find_arch_file "$source_root" "$arch" libSVBCameraSDK.so)
    sdk_shared_dir=$(dirname -- "$sdk_shared")
    for path in "${sdk_shared_dir}"/libSVBCameraSDK.so*; do
        [[ -e "$path" || -L "$path" ]] || continue
        cp -a "$path" "$arch_dir/"
    done

    usb_shared=$(find_arch_file "$source_root" "$arch" libusb-1.0.so)
    usb_shared_dir=$(dirname -- "$usb_shared")
    for path in "${usb_shared_dir}"/libusb-1.0.so*; do
        [[ -e "$path" || -L "$path" ]] || continue
        cp -a "$path" "$arch_dir/"
    done
}

source_root=$(prepare_source_root "$source_input")
stage="${tmp_dir}/svb"
header=$(find_one "$source_root" SVBCameraSDK.h)

mkdir -p "${stage}/include" "${stage}/lib"
cp -a "$header" "${stage}/include/SVBCameraSDK.h"

for arch in x64 x86 armv8 armv7 armv6; do
    copy_arch_libs "$source_root" "$arch" "${stage}/lib/${arch}"
done

timestamp=$(date +%Y%m%d-%H%M%S)
backup="${dest}.backup.${timestamp}"
mkdir -p "$(dirname -- "$dest")"

if [[ -e "$dest" ]]; then
    cp -a "$dest" "$backup"
    printf 'backup: %s\n' "$backup"
fi

rm -rf "$dest"
mv "$stage" "$dest"
printf 'updated: %s\n' "$dest"

if [[ "$run_check" -eq 1 ]]; then
    cd "$repo_root"
    cargo check --workspace
else
    printf 'skipped cargo check; run cargo check --workspace to refresh bindings.\n'
fi
