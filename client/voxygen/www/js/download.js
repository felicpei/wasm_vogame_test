let db
let objectStore

function downResFile(assetName, md5, cb) {
    let rName = assetName.replace(/\\/g, ".")
    readRes(rName, md5, function (data) {
        if (data) {
            window.rust_func.SetResourceData(rName, data)
            cb()
        }
        else {
            axios({
                method: 'get',
                url: "/assets/" + assetName,
                responseType: 'arraybuffer',
            })
                .then(res => {
                    let bytes = new Uint8Array(res.data)
                    addRes(rName, bytes, md5)
                    window.rust_func.SetResourceData(rName, bytes)
                    cb()
                });
        }
    })
}


function DownAllRes(cb) {
    let funcDown = function () {
        axios({
            method: 'get',
            url: '/assets/md5.json',
            responseType: 'json',
        })
            .then(res => {
                let json = res.data
                let jsonArray = []
                let jsonDownIndex = 0
                let downCount = 0

                let loadOneResCB = function () {

                    downCount = downCount + 1
                    document.getElementById("loading").innerHTML = "加载资源中:" + downCount + "/" + jsonArray.length
                    if (downCount == jsonArray.length) {
                        document.getElementById("loading").innerHTML = ""
                        cb()
                    }
                }

                let loadOneRes;
                loadOneRes = function (isFirst) {
                    if (jsonDownIndex < jsonArray.length) {
                        var data = jsonArray[jsonDownIndex]
                        jsonDownIndex = jsonDownIndex + 1
                        downResFile(data[0], data[1], loadOneRes)

                    }
                    if (!isFirst)
                        loadOneResCB()
                }

                for (var key in json) {
                    //TODO  不支持直接存储 
                    if (!key.endsWith(".xml")) {
                        jsonArray.push([key, json[key]])
                    }
                }
                for (let i = 0; i < 1000; i++) {
                    loadOneRes(true)
                }

            });
    }

    console.log("open indexedDB")
    let request = window.indexedDB.open('mcdata');
    request.onerror = function (event) {
        console.error('数据库打开报错');
    };

    request.onsuccess = function (event) {
        db = request.result;
        console.log('数据库打开成功');
        funcDown()
    };
    request.onupgradeneeded = function (event) {
        db = event.target.result;
        console.log('数据库需要升级');
        if (!db.objectStoreNames.contains('resCache')) {
            db.createObjectStore("resCache", { keyPath: "path" })
        }
    }
}

// function getResFile(assetName, ext) {
//     assetName = assetName + "." + ext
//     let res = cache[assetName]
//     if (res == undefined) {
//         res = ""
//         console.error("没有资源:" + assetName)
//     }
//     else {
//         // console.log(res)
//     }
//     return res
// }





//测试用的 1
setTimeout(function () {
    // console.log(window.rustFunc)
    // window.rustFunc.TestCall();
    // axios({
    //     method: 'get',
    //     url: '/assets/md5.json',
    //     responseType: 'arraybuffer',
    // })
    //     .then(res => {
    //         console.error(res.data)
    //         addRes("aaa",res.data,"");
    //         addRes("bbb","122","");

    //     })
}, 1000);



function getTransaction() {
    return objectStore = db.transaction(['resCache'], 'readwrite') //新建事务，readwrite, readonly(默认), versionchange 
        .objectStore('resCache')
}

function addRes(path, data, md5) {
    getTransaction().put({  // 插入记录
        path: path,
        res: data,
        md5: md5
    });
}


function readRes(path, md5, cb) {
    var request = getTransaction().get(path);

    request.onerror = function (event) {
        console.error('事务失败');
    };

    request.onsuccess = function (event) {
        if (request.result) {
            if (request.result.md5 != md5) {
                console.error('md5不一致 重新获取')
                var req = objectStore.delete(path)
                cb()
            }
            else {
                cb(request.result.res)
            }
        } else {
            cb()
            // console.error('未获得数据记录');
        }
    };
}

