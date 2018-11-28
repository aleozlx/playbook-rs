#!/bin/bash

ID="$TKSTACK_USER"
if [[ -z $ID ]]
then
  exec bash # enter root shell when no identity is specified
fi

RULE="^uid=([0-9]+)\((\w*)\) gid=([0-9]+)\((\w*)\) .*$"

function impersonate {
  # mkdir -p "/home/${username}/workspace"
  echo "${username}:x:${uid}:${gid}:${username},,,:/home/${username}:/bin/bash" >> /etc/passwd
  echo "${groupname}:x:${gid}:" >> /etc/group
  echo "${username} ALL=(ALL) NOPASSWD: ALL" > "/etc/sudoers.d/${username}"
  chmod 0440 "/etc/sudoers.d/${username}"
  chown ${uid}:${gid} "/home/${username}"
  install -o "${username}" -g "${username}" "/bin/tkstack-bashrc.sh" "/home/${username}/.bashrc"
  # echo user=${username} argv="$ARGV"
  if [[ -z $ARGV ]]
  then
    exec gosu ${username} bash
  else
    exec gosu ${username} bash -c "$ARGV"
  fi
}

if [[ $ID =~ $RULE ]]
then
  username="${BASH_REMATCH[2]}"
  groupname="${BASH_REMATCH[4]}"
  uid="${BASH_REMATCH[1]}"
  gid="${BASH_REMATCH[3]}"
  ARGV="$@" # rename cmd arguments
  if [[ $username == "root" ]]
  then
    if [[ -z $ARGV ]]
    then
      exec bash
    else
      exec bash -c "$ARGV" # no need to impersonate root
    fi
  fi
  impersonate
else
  echo "tkstack-start: user info"
fi
