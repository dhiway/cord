#!/bin/sh

api_base="https://api.github.com/repos"

# Function to take 2 git tags/commits and get any lines from commit messages
# that contain something that looks like a PR reference: e.g., (#1234)
sanitised_git_logs() {
  git --no-pager log --pretty=format:"%s" "$1...$2" |
    # Only find messages referencing a PR
    grep -E '\(#[0-9]+\)' |
    # Strip any asterisks
    sed 's/^* //g'
}

# Checks whether a tag on github has been verified
# repo: 'organization/repo'
# tagver: 'v1.2.3'
# Usage: check_tag $repo $tagver
check_tag() {
  repo=$1
  tagver=$2
  if [ -n "$GITHUB_RELEASE_TOKEN" ]; then
    echo '[+] Fetching tag using privileged token'
    tag_out=$(curl -H "Authorization: token $GITHUB_RELEASE_TOKEN" -s "$api_base/$repo/git/refs/tags/$tagver")
  else
    echo '[+] Fetching tag using unprivileged token'
    tag_out=$(curl -H "Authorization: token $GITHUB_PR_TOKEN" -s "$api_base/$repo/git/refs/tags/$tagver")
  fi
  tag_sha=$(echo "$tag_out" | jq -r .object.sha)
  object_url=$(echo "$tag_out" | jq -r .object.url)
  if [ "$tag_sha" = "null" ]; then
    return 2
  fi
  echo "[+] Tag object SHA: $tag_sha"
  verified_str=$(curl -H "Authorization: token $GITHUB_RELEASE_TOKEN" -s "$object_url" | jq -r .verification.verified)
  if [ "$verified_str" = "true" ]; then
    # Verified, everything is good
    return 0
  else
    # Not verified. Bad juju.
    return 1
  fi
}

# Checks whether a given PR has a given label.
# repo: 'organization/repo'
# pr_id: 12345
# label: B1-silent
# Usage: has_label $repo $pr_id $label
has_label() {
  repo="$1"
  pr_id="$2"
  label="$3"

  # These will exist if the function is called in Gitlab.
  # If the function's called in Github, we should have GITHUB_ACCESS_TOKEN set
  # already.
  if [ -n "$GITHUB_RELEASE_TOKEN" ]; then
    GITHUB_TOKEN="$GITHUB_RELEASE_TOKEN"
  elif [ -n "$GITHUB_PR_TOKEN" ]; then
    GITHUB_TOKEN="$GITHUB_PR_TOKEN"
  fi

  out=$(curl -H "Authorization: token $GITHUB_TOKEN" -s "$api_base/$repo/pulls/$pr_id")
  [ -n "$(echo "$out" | tr -d '\r\n' | jq ".labels | .[] | select(.name==\"$label\")")" ]
}

github_label() {
  echo
  echo "# run github-api job for labeling it ${1}"
  curl -sS -X POST \
    -F "token=${CI_JOB_TOKEN}" \
    -F "ref=master" \
    -F "variables[LABEL]=${1}" \
    -F "variables[PRNO]=${CI_COMMIT_REF_NAME}" \
    -F "variables[PROJECT]=dhiway/cord" \
    "${GITLAB_API}/projects/${GITHUB_API_PROJECT}/trigger/pipeline"
}

# Pretty-printing functions
boldprint() { printf "|\n| \033[1m%s\033[0m\n|\n" "${@}"; }
boldcat() {
  printf "|\n"
  while read -r l; do printf "| \033[1m%s\033[0m\n" "${l}"; done
  printf "|\n"
}

# Fetches the tag name of the latest release from a repository
# repo: 'organisation/repo'
# Usage: latest_release 'dhiway/cord'
latest_release() {
  curl -s "$api_base/$1/releases/latest" | jq -r '.tag_name'
}

# Check for runtime changes between two commits. This is defined as any changes
# to /primitives/src/* and any chains under /runtime
has_runtime_changes() {
  from=$1
  to=$2

  if git diff --name-only "${from}...${to}" |
    grep -q -e '^runtime/braid' -e '^runtime/loom' -e '^runtime/weave' -e '^primitives/cord' -e '^runtime/common'; then
    return 0
  else
    return 1
  fi
}

# Assumes the ENV are set:
# - RELEASE_ID
# - GITHUB_TOKEN
# - REPO in the form dhiway/cord
fetch_release_artifacts() {
  echo "Release ID : $RELEASE_ID"
  echo "Repo       : $REPO"
  echo "Binary     : $BINARY"
  OUTPUT_DIR=${OUTPUT_DIR:-"./release-artifacts/${BINARY}"}
  echo "OUTPUT_DIR : $OUTPUT_DIR"

  echo "Fetching release info..."
  curl -L -s \
    -H "Accept: application/vnd.github+json" \
    -H "Authorization: Bearer ${GITHUB_TOKEN}" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    https://api.github.com/repos/${REPO}/releases/${RELEASE_ID} > release.json

  echo "Extract asset ids..."
  ids=($(jq -r '.assets[].id' < release.json ))
  echo "Extract asset count..."
  count=$(jq '.assets|length' < release.json )

  # Fetch artifacts
  mkdir -p "$OUTPUT_DIR"
  pushd "$OUTPUT_DIR" > /dev/null

  echo "Fetching assets..."
  iter=1
  for id in "${ids[@]}"
  do
      echo " - $iter/$count: downloading asset id: $id..."
      curl -s -OJ -L -H "Accept: application/octet-stream" \
          -H "Authorization: Token ${GITHUB_TOKEN}" \
          "https://api.github.com/repos/${REPO}/releases/assets/$id"
      iter=$((iter + 1))
  done

  pwd
  ls -al --color
  popd > /dev/null
}

relative_parent() {
    echo "$1" | sed -E 's/(.*)\/(.*)\/\.\./\1/g'
}

# Find all the runtimes, it returns the result as JSON object, compatible to be
# used as Github Workflow Matrix. This call is exposed by the `scan` command and can be used as:
# podman run --rm -it -v /.../fellowship-runtimes:/build docker.io/chevdor/srtool:1.70.0-0.11.1 scan
find_runtimes() {
    libs=($(git grep -I -r --cached --max-depth 20 --files-with-matches '[frame_support::runtime]!' -- '*lib.rs'))
    re=".*-runtime$"
    JSON=$(jq --null-input '{ "include": [] }')

    # EXCLUDED_RUNTIMES is a space separated list of runtime names (without the -runtime postfix)
    # EXCLUDED_RUNTIMES=${EXCLUDED_RUNTIMES:-"substrate-test"}
    IFS=' ' read -r -a exclusions <<< "$EXCLUDED_RUNTIMES"

    for lib in "${libs[@]}"; do
        crate_dir=$(dirname "$lib")
        cargo_toml="$crate_dir/../Cargo.toml"

        name=$(toml get -r $cargo_toml 'package.name')
        chain=${name//-runtime/}

        if [[ "$name" =~ $re ]] && ! [[ ${exclusions[@]} =~ $chain ]]; then
            lib_dir=$(dirname "$lib")
            runtime_dir=$(relative_parent "$lib_dir/..")
            ITEM=$(jq --null-input \
                --arg chain "$chain" \
                --arg name "$name" \
                --arg runtime_dir "$runtime_dir" \
                '{ "chain": $chain, "crate": $name, "runtime_dir": $runtime_dir }')
            JSON=$(echo $JSON | jq ".include += [$ITEM]")
        fi
    done
    echo $JSON
}

# Filter the version matches the particular pattern and return it.
# input: version (v1.8.0 or v1.8.0-rc1)
# output: none
filter_version_from_input() {
  version=$1
  regex="(^v[0-9]+\.[0-9]+\.[0-9]+)$|(^v[0-9]+\.[0-9]+\.[0-9]+-rc[0-9]+)$"

  if [[ $version =~ $regex ]]; then
      if [ -n "${BASH_REMATCH[1]}" ]; then
          echo "${BASH_REMATCH[1]}"
      elif [ -n "${BASH_REMATCH[2]}" ]; then
          echo "${BASH_REMATCH[2]}"
      fi
  else
      echo "Invalid version: $version"
      exit 1
  fi

}

# Check if the release_id is valid number
# input: release_id
# output: release_id or exit 1
check_release_id() {
  input=$1

  release_id=$(echo "$input" | sed 's/[^0-9]//g')

  if [[ $release_id =~ ^[0-9]+$ ]]; then
      echo "$release_id"
  else
      echo "Invalid release_id from input: $input"
      exit 1
  fi

}

# Get latest release tag
#
# input: none
# output: latest_release_tag
get_latest_release_tag() {
    TOKEN="Authorization: Bearer $GITHUB_TOKEN"
    latest_release_tag=$(curl -s -H "$TOKEN" $api_base/dhiway/cord/releases/latest | jq -r '.tag_name')
    printf $latest_release_tag
}

# Enable after setting the GPG keys
# # Check the checksum for a given binary
# check_sha256() {
#     echo "Checking SHA256 for $1"
#     shasum -qc $1.sha256
# }
#
# # Import GPG keys of the release team members
# # This is done in parallel as it can take a while sometimes
# import_gpg_keys() {
#   GPG_KEYSERVER=${GPG_KEYSERVER:-"keyserver.ubuntu.com"}
#   SEC=""
#   EGOR=""
#   MORGAN=""
#
#   echo "Importing GPG keys from $GPG_KEYSERVER in parallel"
#   for key in $SEC $EGOR $MORGAN; do
#     (
#       echo "Importing GPG key $key"
#       gpg --no-tty --quiet --keyserver $GPG_KEYSERVER --recv-keys $key
#       echo -e "5\ny\n" | gpg --no-tty --command-fd 0 --expert --edit-key $key trust;
#     ) &
#   done
#   wait
# }
#
# # Check the GPG signature for a given binary
# check_gpg() {
#     echo "Checking GPG Signature for $1"
#     gpg --no-tty --verify -q $1.asc $1
# }
#
# # GITHUB_REF will typically be like:
# # - refs/heads/release-v1.2.3
# # - refs/heads/release-v1.10.0-rc2
# # This function extracts the version
# function get_version_from_ghref() {
#   GITHUB_REF=$1
#   stripped=${GITHUB_REF#refs/heads/release-}
#   re="v([0-9]+\.[0-9]+\.[0-9]+)"
#   if [[ $stripped =~ $re ]]; then
#     echo ${BASH_REMATCH[0]};
#     return 0
#   else
#     return 1
#   fi
# }


