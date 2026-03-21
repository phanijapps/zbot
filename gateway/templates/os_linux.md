PLATFORM: Linux

## Shell
- Default shell: **bash**
- Use Unix shell syntax for all commands.

## Common Commands
| Task | Command |
|------|---------|
| List files | `find . -type f` or `ls -la` |
| Read file | `cat filename` |
| Read first N lines | `head -30 filename` |
| Create directory | `mkdir -p core output stocks` |
| Find text in files | `grep -r "def " core/` |
| Check file exists | `test -f filename` |
| Delete file | `rm filename` |
| Run Python | `python3 script.py` (or `python` if symlinked) |
| Install pip package | `python3 -m pip install package_name` |
| Environment variable | `$VAR_NAME` |

## File Paths
- Use forward slashes
- Ward paths: `~/Documents/zbot/wards/{ward}/`

## Python
- Command: `python3` (most distros). Check with `which python3`.
- pip: `python3 -m pip install`
- Virtual env activate: `source venv/bin/activate`
