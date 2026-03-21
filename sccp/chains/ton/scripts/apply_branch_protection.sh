#!/usr/bin/env bash
set -euo pipefail

if ! command -v gh >/dev/null 2>&1; then
  echo "apply_branch_protection.sh requires gh in PATH" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "apply_branch_protection.sh requires jq in PATH" >&2
  exit 1
fi

REPO=""
BRANCH=""
DRY_RUN=0
REQUIRE_CODE_OWNER_REVIEWS=0
APPROVAL_COUNT=1

require_value() {
  local flag="$1"
  local value="${2-}"
  if [[ -z "${value}" || "${value}" == --* ]]; then
    echo "Missing value for ${flag}" >&2
    exit 1
  fi
  printf '%s' "${value}"
}

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --repo)
      REPO="$(require_value --repo "${2-}")"
      shift 2
      ;;
    --branch)
      BRANCH="$(require_value --branch "${2-}")"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --require-code-owner-reviews)
      REQUIRE_CODE_OWNER_REVIEWS=1
      shift
      ;;
    --no-require-code-owner-reviews)
      REQUIRE_CODE_OWNER_REVIEWS=0
      shift
      ;;
    --approvals)
      APPROVAL_COUNT="$(require_value --approvals "${2-}")"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1" >&2
      echo "Usage: $0 [--repo owner/name] [--branch name] [--dry-run] [--approvals N] [--require-code-owner-reviews|--no-require-code-owner-reviews]" >&2
      exit 1
      ;;
  esac
done

if ! [[ "${APPROVAL_COUNT}" =~ ^[0-9]+$ ]]; then
  echo "[branch-protection] approvals must be a non-negative integer" >&2
  exit 1
fi

case "${REQUIRE_CODE_OWNER_REVIEWS}" in
  1 | true | TRUE)
    REQUIRE_CODE_OWNER_REVIEWS_JSON=true
    ;;
  0 | false | FALSE)
    REQUIRE_CODE_OWNER_REVIEWS_JSON=false
    ;;
  *)
    echo "[branch-protection] require-code-owner-reviews must be 0/1/true/false" >&2
    exit 1
    ;;
esac

if [[ -z "${REPO}" ]]; then
  REPO="$(gh repo view --json nameWithOwner -q '.nameWithOwner')"
fi

if [[ -z "${BRANCH}" ]]; then
  BRANCH="$(gh repo view "${REPO}" --json defaultBranchRef -q '.defaultBranchRef.name')"
fi

BRANCH_API_PATH="$(jq -rn --arg v "${BRANCH}" '$v|@uri')"

REQUIRED_CHECKS=(
  "SCCP Formal Assisted / formal_assisted"
  "SCCP CI Lint / lint"
)

checks_json="$(
  printf '%s\n' "${REQUIRED_CHECKS[@]}" |
    jq -R . |
    jq -s .
)"

payload="$(
  jq -n \
    --argjson checks "${checks_json}" \
    --argjson strict true \
    --argjson enforce_admins true \
    --argjson dismiss_stale_reviews true \
    --argjson require_code_owner_reviews "${REQUIRE_CODE_OWNER_REVIEWS_JSON}" \
    --argjson required_approving_review_count "${APPROVAL_COUNT}" \
    --argjson require_last_push_approval false \
    --argjson required_linear_history true \
    --argjson allow_force_pushes false \
    --argjson allow_deletions false \
    --argjson block_creations false \
    --argjson required_conversation_resolution true \
    --argjson lock_branch false \
    --argjson allow_fork_syncing true \
    '{
      required_status_checks: {
        strict: $strict,
        contexts: $checks
      },
      enforce_admins: $enforce_admins,
      required_pull_request_reviews: {
        dismissal_restrictions: {users: [], teams: []},
        dismiss_stale_reviews: $dismiss_stale_reviews,
        require_code_owner_reviews: $require_code_owner_reviews,
        required_approving_review_count: $required_approving_review_count,
        require_last_push_approval: $require_last_push_approval
      },
      restrictions: null,
      required_linear_history: $required_linear_history,
      allow_force_pushes: $allow_force_pushes,
      allow_deletions: $allow_deletions,
      block_creations: $block_creations,
      required_conversation_resolution: $required_conversation_resolution,
      lock_branch: $lock_branch,
      allow_fork_syncing: $allow_fork_syncing
    }'
)"

echo "[branch-protection] repo=${REPO} branch=${BRANCH}"

if [[ "${DRY_RUN}" -eq 1 ]]; then
  echo "[branch-protection] dry-run payload:"
  echo "${payload}" | jq '.'
  exit 0
fi

gh api --method PUT "repos/${REPO}/branches/${BRANCH_API_PATH}/protection" --input - <<<"${payload}" >/dev/null

gh api "repos/${REPO}/branches/${BRANCH_API_PATH}/protection" |
  jq -r '
    "[branch-protection] strict=\(.required_status_checks.strict) checks=\(.required_status_checks.contexts|join(" | ")) approvals=\(.required_pull_request_reviews.required_approving_review_count) code_owner_reviews=\(.required_pull_request_reviews.require_code_owner_reviews) linear=\(.required_linear_history.enabled) no_force_push=\((.allow_force_pushes.enabled|not)) no_delete=\((.allow_deletions.enabled|not)) conversation=\(.required_conversation_resolution.enabled)"
  '
