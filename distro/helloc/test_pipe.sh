while IFS= read -r -d $'\0' file; do
    echo "$file"
done < <(find . -maxdepth 1 -type f -print0)

