#!/bin/sh

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <start-commit> <end-commit>"
    exit 1
fi

START="$1"
END="$2"

git rev-parse --verify "$START" >/dev/null 2>&1 || {
    echo "Invalid commit: $START"
    exit 1
}

git rev-parse --verify "$END" >/dev/null 2>&1 || {
    echo "Invalid commit: $END"
    exit 1
}

git log --oneline --reverse "${START}^..${END}"