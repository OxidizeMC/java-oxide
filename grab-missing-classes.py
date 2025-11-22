import re

file = open("./gen-test/jog.txt")
text = file.read()
file.close()
classes = []
regex = r'(?m)ERROR: missing class for field(?:\/argument)? type: "(.+)"'

for match in re.finditer(regex, text):
    if match.group(1) not in classes:
        classes.append(match.group(1))

classes.sort()
for clazz in classes:
    print(clazz)
