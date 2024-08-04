#!/usr/bin/env bash

if ! command -v git-cliff &> /dev/null
then
    echo "git-cliff could not be found"
    exit
fi

git cliff -o CHANGELOG.md
