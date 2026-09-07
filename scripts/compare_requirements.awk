# Compare complete requirement bodies with whitespace normalized. Imported source
# text is data; no Markdown is evaluated as shell code.
function trim(s) { gsub(/^[ \t\r\n]+|[ \t\r\n]+$/, "", s); gsub(/[ \t\r\n]+/, " ", s); return s }
function fail(message) { print message > "/dev/stderr"; errors++ }
function end_section() {
  if(delta && operation!="" && !items) fail("empty " operation " section")
  if(delta && operation=="RENAMED" && old!="") fail("unrecognized rename syntax")
}
function flush() {
  if (name=="") return
  if (!delta) current[name]=trim(body)
  else if (operation=="REMOVED") { if(name in current) fail("removed requirement remains: " name) }
  else if (operation=="ADDED" || operation=="MODIFIED") {
    if (!(name in current) || current[name]!=trim(body)) fail("requirement not synchronized: " name)
  } else fail("requirement outside a supported section: " name)
  name=""; body=""
}
FILENAME != previous {
  flush(); delta=(FILENAME==ARGV[2]); previous=FILENAME
}
/^## (ADDED|MODIFIED|REMOVED|RENAMED) Requirements[ \t]*$/ {
  flush(); end_section(); operation=$2; sections++; items=0; next
}
/^### Requirement: / {
  flush(); name=$0; sub(/^### Requirement: /,"",name); name=trim(name); items++; next
}
delta && operation=="RENAMED" && /^- FROM: `### Requirement: .*`$/ {
  old=$0; sub(/^- FROM: `### Requirement: /,"",old); sub(/`$/,"",old); next
}
delta && operation=="RENAMED" && /^- TO: `### Requirement: .*`$/ {
  new=$0; sub(/^- TO: `### Requirement: /,"",new); sub(/`$/,"",new)
  if(old=="" || old in current || !(new in current)) fail("rename not synchronized: " old " -> " new)
  old=""; renames++; items++; next
}
{ if(name!="") body=body " " $0 }
END {
  flush()
  end_section()
  if(!sections) fail("delta has no recognized requirement sections")
  if(operation=="RENAMED" && (!renames || old!="")) fail("unrecognized rename syntax")
  exit(errors>0)
}
