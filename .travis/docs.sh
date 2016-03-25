#!/bin/bash

set -o errexit

git clone --branch gh-pages "https://$TOKEN@github.com/${TRAVIS_REPO_SLUG}.git" deploy_docs
cd deploy_docs

git config user.name "Sean McArthur"
git config user.email "sean.monstar@gmail.com"

if [ "$TRAVIS_TAG" = ""  ]; then
    rm -rf master
    mv ../target/doc ./master
    echo "<meta http-equiv=refresh content=0;url=hyper/index.html>" > ./master/index.html
else
    rm -rf $TRAVIS_TAG
    mv ../target/doc ./$TRAVIS_TAG
    echo "<meta http-equiv=refresh content=0;url=hyper/index.html>" > ./$TRAVIS_TAG/index.html

    latest=$(echo * | tr " " "\n" | sort -V -r | head -n1)
    if [ "$TRAVIS_TAG" = "$latest" ]; then

        echo "<meta http-equiv=refresh content=0;url=$latest/hyper/index.html>" > index.html
    fi
fi


git add -A .
git commit -m "rebuild pages at ${TRAVIS_COMMIT}"
git push --quiet origin gh-pages
