#!/bin/bash

set -o errexit

shopt -s globstar

cargo doc --no-deps

git clone --branch gh-pages "https://$TOKEN@github.com/${TRAVIS_REPO_SLUG}.git" deploy_docs > /dev/null 2>&1
cd deploy_docs


git config user.name "Sean McArthur"
git config user.email "sean.monstar@gmail.com"

# Fake up the date to be ages ago, so that it doesn't pollute the output of
#   git log --graph --all --oneline
DATE=$((`date +%s`-60*60*24*(365*28+7)))

if [ "$TRAVIS_TAG" = ""  ]; then
    rm -rf master
    mv ../target/doc ./master
    echo "<meta http-equiv=refresh content=0;url=hyper/index.html>" > ./master/index.html
fi

git add -A .
GIT_AUTHOR_DATE=$DATE GIT_COMMITTER_DATE=$DATE \
    git commit -m "rebuild pages at ${TRAVIS_COMMIT}"

echo
echo "Pushing docs..."
git push --quiet origin gh-pages > /dev/null 2>&1
echo
echo "Docs published."
echo

