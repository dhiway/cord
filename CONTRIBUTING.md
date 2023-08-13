# CORD project Contribution guidelines

## Development Workflow

We follow most of the details as per the [document here](https://help.github.com/en/github/collaborating-with-issues-and-pull-requests). If you are not aware of the github workflow, it is recommended to go through them before continuing here.


#### Get the Repository setup

0. Fork Repository
   - Fork [cord repository](https://github.com/dhiway/cord/fork).

1. Clone Repository
   - Clone the cord repo freshly from github using below steps.

```
   git clone git@github.com:${username}/cord.git
   cd cord/
   git remote add upstream git@github.com:dhiway/cord.git
```

Above two tasks are one time for the life time. You can continue to use the same repository for all the work in future.

#### Development & Other flows

0. Issue:
   - Make sure there is an issue filed for the task you are working on.
   - If it is not filed, open the issue with all the description.
   - If it is a bug fix, add label "Type:Bug".
   - If it is an RFC, provide all the required documentation along in the Issue (which can later be included in PR).

1. Code:
   - Start coding
   - Build and test locally

2. Keep up-to-date
   - It is critical for developer to be up-to-date with `develop` branch of the repo to be Conflict-Free when PR is opened.
   - Git provides many options to keep up-to-date, below is one of them
```
   git fetch upstream
   git rebase upstream/develop
```
   - It is recommended you keep pushing to your repo every day, so you don't lose any work.

2. Commit Message / PR description:
   - The name of the branch on your personal fork can start with issueNNNN, followed by anything of your choice.
   - PRs continue to have the title of format "component: \<title\>", like it is practiced now.
   - When you open a PR, having a reference Issue for the commit is mandatory in CORD project.
   - Commit message can have, either `Fixes: #NNNN` or `Updates: #NNNN` in a separate line in the commit message.
     - Here, NNNN is the Issue ID in cord repository.
   - Each commit needs the author to have the "Signed-off-by: Name \<email\>" line.
     - Can do this by `-s` option for `git commit`.
   - If the PR is not ready for review, mark the PR as 'Draft'.

3. Tests:
   - All the required smoke tests would be auto-triggered.
   - Ask for help as comment in PR if you have any questions about the process!

4. Review Process:
   - It is mandatory for getting an 'Approved' review from maintainers.
   - Any further discussions can happen as comments in the PR.

5. Making changes:
   - There are 2 approaches to submit changes done after addressing review comments.
     - Commit changes as a new commit on top of the original commits in the branch, and push the changes to same branch (issueNNNN)
     - Commit changes into the same commit with `--amend` option, and do a push to the same branch with `--force` option.

6. Merging:
   - CORD project follows 'Squash and Merge' method
     - This also makes every merge a complete patch, which has passed all tests.
   - The merging of the patch is expected to be done by the maintainers.
     - It can be done when all the tests (smoke and regression) pass.
     - When the PR has 'Approved' flag from corresponding maintainer.


