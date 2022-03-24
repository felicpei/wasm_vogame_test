let db
let objectStore

function DownAllRes(callBack) {

    //请求indexedDB
    console.log("JS: START OPEN indexedDB")

    let request = window.indexedDB.open('mcdata');
    request.onerror = function (event) {
        console.error('JS: indexedDB Open Error');
    };

    request.onsuccess = function (event) {
        db = request.result;
        console.log('JS: indexedDB Open Success');
        startDownload(callBack);
    };

    request.onupgradeneeded = function (event) {
        db = event.target.result;
        console.warn('JS: indexedDB Need Upgrade');
        if (!db.objectStoreNames.contains('resCache')) {
            db.createObjectStore("resCache", { keyPath: "path" })
        }
    }
}

function startDownload(callBack) {
    axios({
        method: 'get',
        url: '/assets/index.json',
        responseType: 'json',
    })
    .then(res => {
        let json = res.data
        let dirArray = json["dirs"]
        let fileArray =  json["files"]
        let downCount = 0
        let loading = document.getElementById("loading");
        
        //先设置文件夹信息
        for (var idx in dirArray) {
            let path = dirArray[idx]
            let rustPath = path.replace(/\\/g, ".")
            window.rust_func.SetResourceDir(rustPath)
        }
       
        let loadover = function () {
            downCount = downCount + 1;
            loading.innerHTML = "加载资源中:" + downCount + "/" + fileArray.length;
            if (downCount == fileArray.length) {
                loading.innerHTML = ""
                callBack()
            }
        }

         //读取文件信息
         for (var idx in fileArray) {
            let path = fileArray[idx]
            downResFile(path, loadover);
        }
    });
}


function downResFile(assetName, callback) {
    let rName = assetName.replace(/\\/g, ".")
    requestRes(rName, function (data) {

        //尝试读取缓存
        if (data) {
            window.rust_func.SetResourceData(rName, data)
            callback()
        }
        else {
            axios({
                method: 'get',
                url: "/assets/" + assetName,
                responseType: 'arraybuffer',
            })
            .then(res => {
                let bytes = new Uint8Array(res.data)

                //插入缓存
                let objectStore = getStore();
                objectStore.put({ 
                    path: rName,
                    res: bytes,
                });

                //通知rust
                window.rust_func.SetResourceData(rName, bytes)
                callback()
            });
        }
    })
}

function requestRes(rName, callback) {

    let objectStore = getStore();
    var request = objectStore.get(rName);

    request.onerror = function (event) {
        console.error('JS: readRes error');
    };

    request.onsuccess = function (event) {
        if (request.result) {
            callback(request.result.res)
        } else {
            callback()
        }
    };
}

function getStore() {
    let objectStore = db.transaction(['resCache'], 'readwrite').objectStore('resCache')
    return objectStore
}