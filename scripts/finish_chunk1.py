#!/usr/bin/env python3
"""Finish Chunk 1 (Skeleton): check CI, merge the branch into main, add the licence
files, switch on GitHub Pages, and watch the deploy.

Run it from inside your clone of the repo (Windows, Mac or Linux):

    python scripts/finish_chunk1.py            # asks before each change
    python scripts/finish_chunk1.py --yes      # no questions
    python scripts/finish_chunk1.py --dry-run  # only shows what it would do
    python scripts/finish_chunk1.py --run      # also starts the app at the end
    python scripts/finish_chunk1.py --tag v0.0.1   # also tag a release

Needs: git, the GitHub CLI (`gh`, logged in with `gh auth login`), and Python 3.8+.
It never force-pushes and never rewrites history. If anything looks wrong it stops.
"""
import argparse, json, subprocess, sys, time, urllib.request

REPO = "thriple/prism"
BRANCH = "claude/blissful-cerf-jpeibx"
APACHE_URL = "https://www.apache.org/licenses/LICENSE-2.0.txt"

MIT = """MIT License

Copyright (c) {year} {holder}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"""

args = None
# Use your own git identity if set; otherwise a placeholder, so a fresh Windows install still works.
GIT_ID = ["git"]
def _ensure_identity():
    global GIT_ID
    name = subprocess.run(["git", "config", "user.name"], text=True, capture_output=True).stdout.strip()
    email = subprocess.run(["git", "config", "user.email"], text=True, capture_output=True).stdout.strip()
    if not name or not email:
        GIT_ID = ["git", "-c", f"user.name={name or 'Prism owner'}", "-c", f"user.email={email or 'noreply@users.noreply.github.com'}"]


def run(cmd, check=True, capture=True):
    r = subprocess.run(cmd, text=True, capture_output=capture)
    if check and r.returncode != 0:
        out = (r.stdout or "") + (r.stderr or "")
        stop(f"Command failed: {' '.join(cmd)}\n{out.strip()}")
    return (r.stdout or "").strip()


def stop(msg):
    print(f"\nSTOPPED: {msg}")
    sys.exit(1)


def step(msg):
    print(f"\n== {msg}")


def ok(msg):
    print(f"   ok: {msg}")


def confirm(question):
    if args.dry_run:
        print(f"   [dry run] would: {question}")
        return False
    if args.yes:
        return True
    return input(f"   {question} [y/N] ").strip().lower() == "y"


def gh_api(path, *extra, check=True):
    return run(["gh", "api", f"repos/{REPO}/{path}", *extra], check=check)


def main():
    global args
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--yes", action="store_true", help="do not ask before changes")
    ap.add_argument("--dry-run", action="store_true", help="show what would happen, change nothing")
    ap.add_argument("--run", action="store_true", help="start the app at the end (needs Rust)")
    ap.add_argument("--open", action="store_true", help="open the web page and the releases page in your browser at the end")
    ap.add_argument("--tag", help="also create and push this tag, e.g. v0.0.1")
    ap.add_argument("--holder", help="name for the licence copyright line (default: your git user.name)")
    args = ap.parse_args()

    if not args.yes and not args.dry_run:
        print("This will: check that CI passed, merge the Skeleton branch into main, add the MIT and\n"
              "Apache licence files, push main, and switch on GitHub Pages. Nothing is force-pushed.")
        if input("Go ahead? [y/N] ").strip().lower() != "y":
            sys.exit(0)
        args.yes = True

    step("Checking tools")
    for tool in (["git", "--version"], ["gh", "--version"]):
        try:
            run(tool)
        except FileNotFoundError:
            stop(f"'{tool[0]}' is not installed. Install it and try again (gh: https://cli.github.com).")
    run(["gh", "auth", "status"])
    ok("git and gh are ready")
    remote = run(["git", "remote", "get-url", "origin"])
    if REPO not in remote:
        stop(f"This folder's origin is {remote}, not {REPO}. Run the script inside your prism clone.")
    if run(["git", "status", "--porcelain"]):
        stop("You have uncommitted changes here. Commit or stash them first.")

    _ensure_identity()

    step("Looking at the Skeleton branch")
    run(["git", "fetch", "origin"])
    ls = run(["git", "ls-remote", "origin", f"refs/heads/{BRANCH}"], check=False)
    sha = ls.split()[0] if ls else ""
    if not sha:
        stop(f"Branch {BRANCH} is not on GitHub. Has it already been merged and deleted? Check `git log origin/main`.")
    run(["git", "fetch", "origin", f"{BRANCH}:refs/remotes/origin/{BRANCH}"])
    ok(f"branch is at {sha[:8]}")

    step("Checking that CI passed on that exact commit")
    runs = json.loads(gh_api(f"actions/runs?head_sha={sha}&per_page=20"))["workflow_runs"]
    runs = [r for r in runs if r["name"] == "CI"]
    if not runs:
        stop("No CI run found for that commit yet.")
    latest = runs[0]
    while latest["status"] != "completed":
        print(f"   CI is {latest['status']}... waiting 20 s")
        time.sleep(20)
        latest = json.loads(gh_api(f"actions/runs/{latest['id']}"))
    if latest["conclusion"] != "success":
        stop(f"CI finished as '{latest['conclusion']}'. See {latest['html_url']}. Ask Claude Code to fix it.")
    ok("CI is green")

    step("Merging the branch into main")
    run(["git", "checkout", "main"])
    run(["git", "pull", "--ff-only", "origin", "main"])
    already = subprocess.run(["git", "merge-base", "--is-ancestor", f"origin/{BRANCH}", "main"]).returncode == 0
    if already:
        ok("already merged, nothing to do")
    elif confirm(f"merge origin/{BRANCH} into main (a normal merge commit, no force)?"):
        r = subprocess.run(GIT_ID + ["merge", "--no-ff", "-m", "Merge Chunk 1: Skeleton", f"origin/{BRANCH}"],
                           text=True, capture_output=True)
        if r.returncode != 0:
            details = ((r.stdout or "") + (r.stderr or "")).strip()
            merging = subprocess.run(["git", "rev-parse", "-q", "--verify", "MERGE_HEAD"],
                                     capture_output=True).returncode == 0
            if merging:
                subprocess.run(["git", "merge", "--abort"])
                stop("The merge has real conflicts, so nothing was changed. Ask Claude Code to merge main "
                     "into the branch and fix them.\n\ngit said:\n" + details)
            stop("git could not do the merge (this is not a conflict). git said:\n" + details)
        ok("merged")

    step("Licence files")
    import os, datetime
    if os.path.exists("LICENSE-MIT") and os.path.exists("LICENSE-APACHE"):
        ok("already there")
    else:
        holder = args.holder or run(["git", "config", "user.name"], check=False) or "The Prism authors"
        print(f"   copyright holder will be: {holder}  (change with --holder \"Name\")")
        if confirm("add LICENSE-MIT and LICENSE-APACHE?"):
            with open("LICENSE-MIT", "w", newline="\n") as f:
                f.write(MIT.format(year=datetime.date.today().year, holder=holder))
            text = urllib.request.urlopen(APACHE_URL, timeout=30).read().decode("utf-8")
            with open("LICENSE-APACHE", "w", newline="\n") as f:
                f.write(text)
            run(["git", "add", "LICENSE-MIT", "LICENSE-APACHE"])
            run(GIT_ID + ["commit", "-m", "Add MIT and Apache-2.0 licence files"])
            ok("added")

    step("Pushing main")
    if confirm("push main to GitHub?"):
        run(["git", "push", "origin", "main"])
        ok("pushed")

    step("GitHub Pages")
    pages = gh_api("pages", check=False)
    if pages and '"build_type":"workflow"' in pages.replace(" ", ""):
        ok("already set to deploy from GitHub Actions")
    elif confirm("set Pages > Source to GitHub Actions?"):
        if pages:
            gh_api("pages", "-X", "PUT", "-f", "build_type=workflow")
        else:
            gh_api("pages", "-X", "POST", "-f", "build_type=workflow")
        ok("Pages enabled")
        # Re-run the newest CI on main so the deploy job runs now that Pages exists.
        newest = json.loads(gh_api("actions/runs?branch=main&per_page=5"))["workflow_runs"]
        newest = [r for r in newest if r["name"] == "CI"]
        if newest:
            subprocess.run(["gh", "run", "rerun", str(newest[0]["id"]), "-R", REPO])

    if args.tag and confirm(f"create and push tag {args.tag} (starts the release build)?"):
        run(["git", "tag", args.tag])
        run(["git", "push", "origin", args.tag])
        ok(f"tag {args.tag} pushed. Release files appear at https://github.com/{REPO}/releases")

    if not args.dry_run:
        step("Watching the web deploy")
        for _ in range(30):
            time.sleep(20)
            newest = json.loads(gh_api("actions/runs?branch=main&per_page=5"))["workflow_runs"]
            newest = [r for r in newest if r["name"] == "CI"]
            if newest and newest[0]["status"] == "completed":
                print(f"   CI on main: {newest[0]['conclusion']}")
                break
            print("   still running...")
        print("   Web page (once deployed): https://thriple.github.io/prism/")

    if args.open and not args.dry_run:
        import webbrowser
        webbrowser.open("https://thriple.github.io/prism/")
        webbrowser.open(f"https://github.com/{REPO}/releases")

    if args.run:
        step("Starting the app (close the window to finish)")
        subprocess.run(["cargo", "run", "--release"])

    print("\nDone. Still for you to do by hand:")
    print("  - Web page: https://thriple.github.io/prism/ should show a dark window with a frame counter")
    print("    that moves, and mouse and key events listed. (Pages can take a few minutes to appear.)")
    print("  - Windows app: after a few minutes the Releases page has a Windows zip. Unzip it and run the .exe.")
    print("  - iPad: open the web page address in Chrome there and tell Claude Code whether it runs.")
    print("  - Decide the IBM Plex fonts question (OFL licence) and tell Claude Code.")


if __name__ == "__main__":
    main()
