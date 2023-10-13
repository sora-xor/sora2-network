// The script that posts comments to PRs on gh
module.exports = async ({ github, context, header, body }) => {
    const comment = [header, body].join("\n");

    const pr_number = await get_pr_number(github, context);

    const { data: comments } = await github.rest.issues.listComments({
        owner: context.repo.owner,
        repo: context.repo.repo,
        issue_number: pr_number,
    });

    const botComment = comments.find(
        (comment) =>
            // github-actions bot user
            comment.user.id === 41898282 && comment.body.startsWith(header)
    );

    const commentFn = botComment ? "updateComment" : "createComment";

    await github.rest.issues[commentFn]({
        owner: context.repo.owner,
        repo: context.repo.repo,
        body: comment,
        ...(botComment
            ? { comment_id: botComment.id }
            : { issue_number: pr_number }),
    });
};

// Returns gh PR number
async function get_pr_number(github, context) {
    if (context.issue.number) {
        // Return issue number if present
        return context.issue.number;
    } else {
        // Otherwise return issue number from commit
        return (
            await github.rest.repos.listPullRequestsAssociatedWithCommit({
                commit_sha: context.sha,
                owner: context.repo.owner,
                repo: context.repo.repo,
            })
        ).data[0].number;
    }
}