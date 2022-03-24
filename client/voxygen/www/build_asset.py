import os
import hashlib
import json

dir = os.getcwd()
result = {}
result["dirs"] = []
result["files"] = []

#按照目录结构生成index
for root, dirs, files in os.walk(dir + "\\assets"):
    if(root.find("assets\\server")>1):
        continue

    for file in files:
        fullPath = root+'\\'+file
        path = fullPath.replace(dir+'\\assets\\', '') 
        result["files"].append(path)

    for p in dirs:
        dirFullPath = root+'\\'+ p
        dirPath = dirFullPath.replace(dir + '\\assets\\', '')
        result["dirs"].append(dirPath)


    # for file in files:
    #     fullPath = root+'\\'+file
    #     path = fullPath.replace(dir+'\\assets\\', '')
    #     md5 = getMd5(fullPath)
    #     dic[path] = md5

# def getMd5(path):
#     fp = open(path, "rb")
#     contents = fp.read()
#     fp.close()
#     return hashlib.md5(contents).hexdigest()


fp= open(dir+"\\assets\\index.json",'w')
fp.write(json.dumps(result))
fp.close()
