Output demo.txt
Output demo.gif
Output demo.mp4
Set PlaybackSpeed .5 # half speed for the videos
Hide

Require git
Require hx
Require cconvention
# FIXME: figure out how to alias build outputs as cconvention

Type@0ms "temp_dir=$(./setup.sh)"
Enter

Type@0ms "cd $temp_dir"
Enter

Type@0ms "clear"
Enter

Type@0ms "export EDITOR=hx"
Enter
Type@0ms "git --no-pager status --short"
Enter

Show

Type "git commit"
Enter

Type "i" # enter insert mode
Type "fe" # start a type
Sleep 500ms # admire the completion
Tab # complete the type
Type "(" # open the scope
Sleep 200ms # admire the error message

# TODO: use completion here?
Type "scope):"
Escape # exit insert mode
Type ":w" # demo autoformatting
Enter
Type "i"
Type "description of change"
Enter
Type "longer description of change"
Enter
Escape
Type ":w"
Enter
Type "i"

Type "BREAKING-CHANGE: ..."
Enter
Escape
Type ":w"
Enter
Type ":q"
