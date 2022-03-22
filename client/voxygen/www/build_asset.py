import os
import hashlib
import json

dir = os.getcwd()
dic = {}


def getMd5(path):
    fp = open(path, "rb")
    contents = fp.read()
    fp.close()
    return hashlib.md5(contents).hexdigest()

for root, dirs, files in os.walk(dir+"\\assets"):
    if(root.find("assets\\server")>1):
        continue
    for file in files:
        fullPath = root+'\\'+file
        path = fullPath.replace(dir+'\\assets\\', '')
        md5 = getMd5(fullPath)
        dic[path] = md5

md5Json = json.dumps(dic)
fp= open(dir+"\\assets\\md5.json",'w')
fp.write(md5Json)
fp.close()
