Please add, commit and push our changes.

IMPORTANT: The shell is PowerShell on Windows. Do NOT use bash heredoc syntax (<<'EOF') for commit messages. Instead, use PowerShell string variables with backtick-n (`n) for newlines:

```powershell
$msg = "Subject line`n`nBody text`n`nCo-Authored-By: MatterHackers <noreply@matterhackers.com>"; git commit -m $msg
```