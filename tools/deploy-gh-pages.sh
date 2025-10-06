#!/usr/bin/env bash

set -e

# VeGen GitHub Pages Deployment Script

deploy_gh_pages() {
    git worktree add gh-pages

    # Ensure cleanup on exit
    cleanup() {
        echo "Cleaning up worktree..."
        git worktree remove gh-pages
    }
    trap cleanup EXIT

    # Copy required files to temp directory
    echo "Copying files into generated site temp directory..."
    cp -r docs/web/* gh-pages/
    cp README.md gh-pages/
    mkdir -p gh-pages/docs/img
    cp docs/img/logo.png gh-pages/docs/img/logo.png

    # compile todo demo
    pushd examples/todo >/dev/null
        echo "Compiling todo example..."
        cargo run src/todo.vg -o src/todo.ts
        npm run build-todo-base
    popd >/dev/null
    rm -fR gh-pages/todo
    cp -r examples/todo/dist gh-pages/todo

    # Prepare git worktree for gh-pages branch without switching current branch
    echo "Preparing git worktree for gh-pages branch..."

    # Commit changes inside the worktree without touching the current branch
    echo "Committing changes in gh-pages worktree..."
    pushd gh-pages >/dev/null
        git add --all

        commit_message="Deploy documentation to gh-pages - $(date)"
        # Only commit if there are staged changes
        if ! git diff --cached --quiet; then
            git commit -m "$commit_message"
            echo "Committed changes to gh-pages branch in worktree"
        else
            echo "No changes to commit in gh-pages worktree"
        fi
    popd >/dev/null

    echo "Deployment complete!"
}

# Run the deployment function
deploy_gh_pages "$@"
