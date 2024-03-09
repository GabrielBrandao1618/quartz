tag=$1

if [ -z "$tag" ]; then
  echo "Usage: rollback.sh <tag>"
  exit 1
fi

git tag --delete $tag
git reset --hard HEAD~
git push --delete origin $tag
gh release delete
gh release delete $tag
git push --force
