#!/usr/bin/env bash
set -euo pipefail

if ! command -v gh >/dev/null 2>&1; then
  echo "check_branch_protection.sh requires gh in PATH" >&2
  exit 1
fi

require_value() {
  local flag="$1"
  local value="${2:-}"
  if [[ -z "${value}" || "${value}" == --* ]]; then
    echo "missing value for ${flag}" >&2
    echo "Usage: $0 [--repo owner/name] [--branch name] [--approvals N] [--require-code-owner-reviews|--no-require-code-owner-reviews]" >&2
    exit 1
  fi
}

REPO=""
BRANCH=""
EXPECTED_APPROVALS=1
EXPECTED_REQUIRE_CODE_OWNER_REVIEWS=0

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --repo)
      require_value "$1" "${2:-}"
      REPO="${2:-}"
      shift 2
      ;;
    --branch)
      require_value "$1" "${2:-}"
      BRANCH="${2:-}"
      shift 2
      ;;
    --approvals)
      require_value "$1" "${2:-}"
      EXPECTED_APPROVALS="${2:-}"
      shift 2
      ;;
    --require-code-owner-reviews)
      EXPECTED_REQUIRE_CODE_OWNER_REVIEWS=1
      shift
      ;;
    --no-require-code-owner-reviews)
      EXPECTED_REQUIRE_CODE_OWNER_REVIEWS=0
      shift
      ;;
    *)
      echo "Unknown argument: $1" >&2
      echo "Usage: $0 [--repo owner/name] [--branch name] [--approvals N] [--require-code-owner-reviews|--no-require-code-owner-reviews]" >&2
      exit 1
      ;;
  esac
done

if [[ -z "${REPO}" ]]; then
  REPO="$(gh repo view --json nameWithOwner -q '.nameWithOwner')"
fi

if [[ -z "${BRANCH}" ]]; then
  BRANCH="$(gh repo view "${REPO}" --json defaultBranchRef -q '.defaultBranchRef.name')"
fi

if ! [[ "${EXPECTED_APPROVALS}" =~ ^[0-9]+$ ]]; then
  echo "[branch-protection-check] expected approvals must be a non-negative integer" >&2
  exit 1
fi

case "${EXPECTED_REQUIRE_CODE_OWNER_REVIEWS}" in
  1 | true | TRUE)
    EXPECTED_REQUIRE_CODE_OWNER_REVIEWS_BOOL=true
    ;;
  0 | false | FALSE)
    EXPECTED_REQUIRE_CODE_OWNER_REVIEWS_BOOL=false
    ;;
  *)
    echo "[branch-protection-check] expected require-code-owner-reviews must be 0/1/true/false" >&2
    exit 1
    ;;
esac

EXPECTED_CHECKS=(
  "SCCP CI Lint / lint"
  "SCCP Formal Assisted / formal_assisted"
)
EXPECTED_CHECKS_JOINED="$(
  printf '%s\n' "${EXPECTED_CHECKS[@]}" |
    sort |
    paste -sd'|' -
)"

protection_json="$(gh api "repos/${REPO}/branches/${BRANCH}/protection")"

actual_checks_joined="$(
  jq -r '.required_status_checks.contexts | sort | join("|")' <<<"${protection_json}"
)"
actual_strict="$(jq -r '.required_status_checks.strict' <<<"${protection_json}")"
actual_approvals="$(jq -r '.required_pull_request_reviews.required_approving_review_count' <<<"${protection_json}")"
actual_require_code_owner_reviews="$(jq -r '.required_pull_request_reviews.require_code_owner_reviews' <<<"${protection_json}")"
actual_linear="$(jq -r '.required_linear_history.enabled' <<<"${protection_json}")"
actual_no_force_push="$(
  jq -r '(.allow_force_pushes.enabled | not)' <<<"${protection_json}"
)"
actual_no_delete="$(
  jq -r '(.allow_deletions.enabled | not)' <<<"${protection_json}"
)"
actual_conversation_resolution="$(jq -r '.required_conversation_resolution.enabled' <<<"${protection_json}")"
actual_enforce_admins="$(jq -r '.enforce_admins.enabled' <<<"${protection_json}")"

fail() {
  echo "[branch-protection-check] $1" >&2
  exit 1
}

[[ "${actual_checks_joined}" == "${EXPECTED_CHECKS_JOINED}" ]] || fail "required checks mismatch (actual='${actual_checks_joined}', expected='${EXPECTED_CHECKS_JOINED}')"
[[ "${actual_strict}" == "true" ]] || fail "strict status checks are not enabled"
[[ "${actual_approvals}" == "${EXPECTED_APPROVALS}" ]] || fail "approval count mismatch (actual='${actual_approvals}', expected='${EXPECTED_APPROVALS}')"
[[ "${actual_require_code_owner_reviews}" == "${EXPECTED_REQUIRE_CODE_OWNER_REVIEWS_BOOL}" ]] || fail "require_code_owner_reviews mismatch (actual='${actual_require_code_owner_reviews}', expected='${EXPECTED_REQUIRE_CODE_OWNER_REVIEWS_BOOL}')"
[[ "${actual_linear}" == "true" ]] || fail "required_linear_history is not enabled"
[[ "${actual_no_force_push}" == "true" ]] || fail "force push is allowed"
[[ "${actual_no_delete}" == "true" ]] || fail "branch deletion is allowed"
[[ "${actual_conversation_resolution}" == "true" ]] || fail "required_conversation_resolution is not enabled"
[[ "${actual_enforce_admins}" == "true" ]] || fail "enforce_admins is not enabled"

echo "[branch-protection-check] OK repo=${REPO} branch=${BRANCH}"
