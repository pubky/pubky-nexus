CREATE CONSTRAINT uniqueUserId FOR (u:User) REQUIRE u.id IS UNIQUE;
CREATE CONSTRAINT uniquePostId FOR (p:Post) REQUIRE p.id IS UNIQUE;
CREATE CONSTRAINT uniquePostUri FOR (p:Post) REQUIRE p.uri IS UNIQUE;
MERGE (u:User {id: "1t5fyryjggt6kfha18jcstnz3wfyqh9fycra5sg8hr5qhmot5rky"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "3iwsuz58pgrf7nw4kx8mg3fib1kqyi4oxqmuqxzsau1mpn5weipo"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "3qgon1apkcmp63xbqpkrb3zzrja3nq9wou4u5bf7uu8rc9ehfo3y"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "3s88s3b9ik7pg3s4s3u48enp3kbweaydx33fsgd6tnrosaxz6dfy"}) SET u.name = "teste", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "4b3xhs34k1c8xbem1tj9phr4nf8xkn6w1eckkie3gipmgsfsbw6y"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "4nacrqeuwh35kwrziy4m376uuyi7czazubgtyog4adm77ayqigxo"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "4p1qa1ko7wuta4f1qm8io495cqsmefbgfp85wtnm9bj55gqbhjpo"}) SET u.name = "Intruder", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}) SET u.name = "Aldert", u.bio = "Lead Designer & Brand Manager. Building meaningful products and brands that empower society. Accelerating hyperbitcoinization at Synonym.", u.status = "working", u.links = "[{\"title\":\"website\",\"url\":\"https://www.synonym.to\"},{\"title\":\"x\",\"url\":\"https://x.com/aldert\"}]";
MERGE (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}) SET u.name = "Jared Cassin", u.bio = "Vis reprehenderit tabgo audentia suffoco curis voluptatum coruscus.", u.status = "undefined", u.links = "[{\"url\":\"https://loving-drive.net/\",\"title\":\"website\"}]";
MERGE (u:User {id: "5ddrprkjm19mz8rokgnqgisommz3zdnfz1yhg1is9kmaoujwrsby"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "5f4e8eoogmkhqeyo5ijdix3ma6rw9byj8m36yrjp78pnxxc379to"}) SET u.name = "arst", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy"}) SET u.name = "Flavio", u.bio = "flavio bio", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"flavio@synonym.to\",\"title\":\"email\"},{\"url\":\"flaviomoceri\",\"title\":\"x\"},{\"url\":\"flaviomoceri\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "6gahxazkp5jk3n69h856gqjoak66xbpybq5c13abnmw3kyhygfty"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "6ramoshwf43ykn3bdfxb1qn9yy7zbrjyknzrycqxh3s59fapukny"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "6xejaazm58f5dca3aj6o4is3k55wxy86hyxtd1pu5h897cfq76yy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "6z6dsqajktysrzmciep3tt8n8y873ccn4zxney1tmh7k51rw1j5o"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "77m8jyqqrd41xzu5b67m8z5wuypuatc54gmw5gcax6ae3yeca6wo"}) SET u.name = "o1er", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}) SET u.name = "Nina Grant", u.bio = "Undique maiores vox sunt et calcar avaritia benigne.", u.status = "undefined", u.links = "[{\"url\":\"https://angelic-gale.com\",\"title\":\"website\"}]";
MERGE (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}) SET u.name = "Terri Conn", u.bio = "Terra decumbo constans blandior succurro strenuus theatrum verbum.", u.status = "undefined", u.links = "[{\"url\":\"https://aged-makeup.net\",\"title\":\"website\"}]";
MERGE (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}) SET u.name = "Alan Bahringer-Spinka", u.bio = "Demens vigilo balbus viscus non succurro suspendo admoneo.", u.status = "undefined", u.links = "[{\"url\":\"https://that-hornet.biz/\",\"title\":\"website\"}]";
MERGE (u:User {id: "7oq5wj1adxk1u94ojh6eknwj3b4z88zcbb51dbam5zn7zeqnzoio"}) SET u.name = "uraj1r0", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "7w4hmktqa7gia5thmk7zki8px7ttwpwjtgaaaou4tbqx64re8d1o"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "8ajb4fbw91fuzywtix3jsc5x416jjpwrue4qricj7k7nt8fjensy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "8gmq7a5cpn8bd57co871ob6txx9hamt1q5gqdsyiotgee58dr4dy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y"}) SET u.name = "Eman Ruoy", u.bio = "", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}) SET u.name = "Terrell Ledner", u.bio = "Velum vacuus territo aperio causa occaecati vinco.", u.status = "undefined", u.links = "[{\"url\":\"https://supportive-clock.info/\",\"title\":\"website\"}]";
MERGE (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}) SET u.name = "Severin Alex Bühler", u.bio = "Tech entrepreneur, ₿ cartographer, Sun fetishist ☀️, Former digital nomad, Creator 
@lnrouter, Engineer 
@Synonym_to", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"@SeverinAlexB\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}) SET u.name = "Kimberly Treutel-Weissnat", u.bio = "Tabernus pecto possimus demergo esse timor.", u.status = "undefined", u.links = "[{\"url\":\"https://selfish-middleman.com/\",\"title\":\"website\"}]";
MERGE (u:User {id: "azmpc34j1pn653dwi3z7rgcsd6xx35eqbhm9fpjonugfcfn68sry"}) SET u.name = "miguel medeiros", u.bio = "", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}) SET u.name = "Luis O'Connell", u.bio = "Vulgus damnatio stipes corrupti excepturi.", u.status = "undefined", u.links = "[{\"url\":\"https://mortified-building.com\",\"title\":\"website\"}]";
MERGE (u:User {id: "bbkdkhm97pytrb785rdpornkjpcxi331hpq446ckn6rhb4abiguy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "bmgm3xk96wqfdnei8jmrxz4ncuyoi99z9tb1ofq79r4biw9a7ffy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "c4yotzcb76d31y44jsymtdcowqg7oyqej46jty3yy7ybtzt9x41o"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "cuimec4ngawamq8wa6fjzki6boxmwqcm11x6g7ontufrjwgdaxqo"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy"}) SET u.name = "peter", u.bio = "", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "dfwfafsmkuiwu78ag4gnzhdb4efg5h4tku4bk6qioxfc6zr9sxjy"}) SET u.name = "New", u.bio = "Fresh account", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "dujh8de33hbaqerg1ejyu9ynjh5yqfozm6iexsjdj3h9yfn3n3wy"}) SET u.name = "test", u.bio = "", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "dzz8cshisfst7dthpy7eio9a3byecmrym1ymn75hwqt67a9fs7zo"}) SET u.name = "JBro", u.bio = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua", u.status = "undefined", u.links = "[{\"url\":\"http://cake.co.uk\",\"title\":\"website\"},{\"url\":\"j@b.co.uk\",\"title\":\"email\"},{\"url\":\"o@\",\"title\":\"x\"},{\"url\":\"t\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso"}) SET u.name = "Matt Carvalho", u.bio = "", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny"}) SET u.name = "123", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "eroud7pzna7aacy5ob6ziekmm3sjg3m8hkpafcdjnwbmxambzyuy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o"}) SET u.name = "aaa", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "f8oaczqycmecdhmz8tbjxsnhb53yk7qrtgeg9kg9hkkbn5ynouoy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "f8r8pf61kh7cpirthz7e1ztqzr8qg5yf7pbnbeifjx4exdqaekpo"}) SET u.name = "dz", u.bio = "dzdidi", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "f9rxf5hu1isngupfe6ff741bh7uqjxjwokqc4u3eribzmi89bcxy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "fh961bofrue6oqupmu6xdqn31sghfjxkefq7ocsf9osub7jn7r9y"}) SET u.name = "Amir", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "fy3ekk5mfm8nje69nwqb7yhou79kjhm41qp35px8o6zcbqc51k5y"}) SET u.name = "Jay", u.bio = "", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "g9biqrydd83a93amx4tyuobws5zehp6n513dgn85hahdbmb4gicy"}) SET u.name = "miguel", u.bio = "", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "ghdxffrmhstihczuny9upgwoqpokw6bk3pbs1bxdioktoz88ar3o"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}) SET u.name = "Opal Jenkins Sr.", u.bio = "Conventus laboriosam spoliatio tabella.", u.status = "undefined", u.links = "[{\"url\":\"https://soft-elf.info\",\"title\":\"website\"}]";
MERGE (u:User {id: "gk9ad7hxtusrf4thnado5rh1on5o1qjwnax1cu6tghgdjck1oiiy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "gonozda78u3wez3r5xnqf8rwiio6xyy5aigqpz3hftmpd5xskidy"}) SET u.name = "JeanChristophe", u.bio = "Hey!", u.status = "undefined", u.links = "[{\"url\":\"https://synonym.to/\",\"title\":\"website\"},{\"url\":\"jean@synonym.to\",\"title\":\"email\"},{\"url\":\"@jc_busnel\",\"title\":\"x\"},{\"url\":\"jeanchristophe\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}) SET u.name = "dzdidi", u.bio = "very short bio", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}) SET u.name = "Flavio Moceri", u.bio = "flavio's bio", u.status = "working", u.links = "[{\"title\":\"Telegram\",\"url\":\"https://t.me/flaviomoceri\"}]";
MERGE (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) SET u.name = "miguel medeiros", u.bio = "dev", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "hmzfwyfb9ezxmng3rc7ohfoumar4p18yeiyb3d91koy1iea48r4y"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "hrrrixk98mzi89c99m9a1e53fxszb6f44nzz3rptt5n6r5j4pxco"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}) SET u.name = "Plebtoshi Ovi", u.bio = "Ovis mean altego.", u.status = "working", u.links = "[{\"title\":\"website\",\"url\":\"https://bitkit.to\"},{\"title\":\"email\",\"url\":\"ovi@synonym.to\"}]";
MERGE (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}) SET u.name = "jay_new", u.bio = "", u.status = "available", u.links = "[]";
MERGE (u:User {id: "i3u8b66j53tgdwt4hyhdwf1ts8rq76qub7skpbgs8nje8upa48by"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}) SET u.name = "PowerfulBTC", u.bio = "", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"pavel@synonym.to\",\"title\":\"email\"},{\"url\":\"manwithpurpose\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}) SET u.name = "Corey", u.bio = "Corey's short bio", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"corey@synonym.to\",\"title\":\"email\"},{\"url\":\"@coreylphillips\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "iruwsoj1zqxbcrq9m1s7jtja48tstae6shmw39s5wbugicxzrtwy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "it788xohmmo7sr8k9knuy56oqtfs7rtc1gixyxdhraqkeaihkdqo"}) SET u.name = "delete me", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "iuw3zijihd68g7xr9txb4ih3bwfz4433zjdhqb8cahg695g6zqxy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}) SET u.name = "Claire Rolfson", u.bio = "Barba amet votum libero facere.", u.status = "undefined", u.links = "[{\"url\":\"https://muddy-mileage.name\",\"title\":\"website\"}]";
MERGE (u:User {id: "jbutqpwpcez6a4mxudcfjyw67dsk3uo3nh8qm1k1m4go1nnjn5ao"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "jn6rpfszntneom7tx5aj4kcn4rid5adfwtsusog5yunpn1ptp86o"}) SET u.name = "SHA256", u.bio = "", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "k46cg1wqtuqx754edku9h1m3k1cgew8or6d93g85nks4fdi6dpwo"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "k74najezunp7k7k5y4dh9u8p59kxnxh4iudp4amdbchx77i9u64o"}) SET u.name = "Foo", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "k851u6ygpfwigbrbdwjers9u6nk3t8bne75kerxd4bpxarsgsthy"}) SET u.name = "Foo2", u.bio = "foo2", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "kft3id7a1krty8gezac7zcj5za9spmwh4mwe775w3fsteo99bn8o"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "kg671gqu6akiyuzdsqgqtupftuhbuwfx5zh6tbygmexuw6b55s4y"}) SET u.name = "Sev", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}) SET u.name = "Sev Number 3", u.bio = "This is my 3rd account 🫠", u.status = "available", u.links = "[]";
MERGE (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}) SET u.name = "Mrs. Ginger Hamill", u.bio = "Coadunatio deorsum earum stabilis provident deduco tego deprimo cenaculum unus.", u.status = "undefined", u.links = "[{\"url\":\"https://gleaming-contrail.biz/\",\"title\":\"website\"}]";
MERGE (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}) SET u.name = "Murray Rothbard", u.bio = "First Pubky Bot.", u.status = "undefined", u.links = "[{\"title\":\"Twitter\",\"type\":\"website\",\"url\":\"https://x.com/murray_rothbot\"}]";
MERGE (u:User {id: "m91ru97xoa5br3bw1bo5hohnz1ttkymwh6tfxzxfouait9epexro"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}) SET u.name = "Janice Stark PhD", u.bio = "Carcer venia victus ocer clementia super asperiores debilito.", u.status = "undefined", u.links = "[{\"url\":\"https://bold-dumbwaiter.info/\",\"title\":\"website\"}]";
MERGE (u:User {id: "mhi9gq35wpqzi4iocuob5nhf4fa4eoppwxnknged4c48q14dk5ho"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "mwpzjenysim7koioqc8qf4ymwgpyi6eotnr9pxuuskh5qndcngzy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "mwsnc3qzej8hks6motdeyj8ag7gzaf3ft5emcjzk9wn5erxg968y"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "nhzo6irmzq34w4ez6rnh618wfz8s6bjgnjw649mt91qm3h5czjao"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "nkmnt9uzjbwzusxjjnrzd4uwd79nhnywitqhj11pannyo7e5aory"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}) SET u.name = "Miguel Medeiros", u.bio = "Memento mori.

PGP:  46A3AC8395F95A6E6D8F1E34819EDEE4673F3EBB", u.status = "working", u.links = "[{\"title\":\"website\",\"url\":\"https://miguelmedeiros.com.br\"},{\"title\":\"twitter\",\"url\":\"https://x.com/_miguelmedeiros\"}]";
MERGE (u:User {id: "ocnjebjbzb75zckt7sdxmr8gjmtj4dba5fi4aznufeq8sobop5oo"}) SET u.name = "YAAA", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}) SET u.name = "Johnnie Altenwerth III", u.bio = "Cupiditas venio vorax amplexus aspernatur quidem concido condico cohors certe.", u.status = "undefined", u.links = "[{\"url\":\"https://rotating-organization.info\",\"title\":\"website\"}]";
MERGE (u:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}) SET u.name = "Flavio Moceri", u.bio = "dfklsdhgklshgkldfhgklfdhkldfklhkdfklhjdfkhjkldfhdjkf", u.status = "undefined", u.links = "[{\"url\":\"https://test.com\",\"title\":\"website\"},{\"url\":\"test@test.com\",\"title\":\"email\"},{\"url\":\"@test\",\"title\":\"x\"},{\"url\":\"test\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "pcckx7sercfy1u8rrr8cc4gkdnce93f6jarngcdsfu5enty51aiy"}) SET u.name = "limpbrains", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco"}) SET u.name = "carson", u.bio = "🇨🇦", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"carson@synonym.to\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}) SET u.name = "Miss Denise Wilderman", u.bio = "Voluptatem aut illo.", u.status = "undefined", u.links = "[{\"url\":\"https://tight-brass.info/\",\"title\":\"website\"}]";
MERGE (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}) SET u.name = "Mary Auer", u.bio = "Usus officia bis accusantium.", u.status = "undefined", u.links = "[{\"url\":\"https://forthright-pastoralist.info\",\"title\":\"website\"}]";
MERGE (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}) SET u.name = "Arturo Mertz", u.bio = "Stipes stipes paens aveho.", u.status = "undefined", u.links = "[{\"url\":\"https://menacing-boss.org\",\"title\":\"website\"}]";
MERGE (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) SET u.name = "Nuh 🔻", u.bio = "strong opinions and emotions, no half measures.

\"horrible BitTorrent mainline person\"", u.status = "noStatus", u.links = "[{\"title\":\"website\",\"url\":\"https://nuh.dev\"}]";
MERGE (u:User {id: "pyc598poqkdgtx1wc4aeptx67mqg71mmywyh7uzkffzittjmbiuo"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "q4iwcxkmxfq86oqnkd597akf5ixp8wwr3es5jxjm66pihnmxugey"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "qnostbczowotpm8twnhjnzruyebjrpyfkf7ehxd1uxqgnjw1aqpo"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "qumq6fady4bmw4w5tpsrj1tg36g3qo4tcfedga9p4bg4so4ikyzy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "r4irb481b8qspaixq1brwre8o87cxybsbk9iwe1f6f9ukrxxs7bo"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "r91hi8kc3x6761gwfiigr7yn6nca1z47wm6jadhw1jbx1co93r9y"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "rz6oe4yda9em9b4m7ymttgym3r9g5gfa51su3rgdj9oszyz787ny"}) SET u.name = "Wobly", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}) SET u.name = "J₿ro", u.bio = "What's testing the tests that test the tests?", u.status = "undefined", u.links = "[{\"url\":\"https://ipfs.io/ipfs/bafybeihkoviema7g3gxyt6la7vd5ho32ictqbilu3wnlo3rs7ewhnp7lly\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "soiaydptr5cgn8p5xge6jwnj1s7t3x9fznoy5unzj4ad37oip4xo"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "sse599pgxr9ahwrrns7yz3dpd118wjjpiisscrh4qnrgj4334iuo"}) SET u.name = "Coolio", u.bio = "Cool before it was cool 🫠", u.status = "undefined", u.links = "[{\"url\":\"https://www.synonym.to\",\"title\":\"website\"},{\"url\":\"ovi@synonym.to\",\"title\":\"email\"},{\"url\":\"@ovitrif\",\"title\":\"x\"},{\"url\":\"ovitrif\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "tkpeqpx3ywoawiw6q8e6kuo9o3egr7fnhx83rudznbrrmqgdmomo"}) SET u.name = "test", u.bio = "test without images", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "tn8xfg7kik7xfuwpmf1kerwdwd1je3xnf914rgyyuprh8c5w4sgo"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "twqjigg3ryhtxbd3uityam8oaib7zwnxm1t1umin343t1a5w7pky"}) SET u.name = "miguel 1", u.bio = "", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho"}) SET u.name = "01er", u.bio = "", u.status = "working", u.links = "[]";
MERGE (u:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}) SET u.name = "Hello", u.bio = "hello", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}) SET u.name = "Sylvester Altenwerth", u.bio = "Tres curatio deripio.", u.status = "undefined", u.links = "[{\"url\":\"https://discrete-palate.com/\",\"title\":\"website\"}]";
MERGE (u:User {id: "w6rookgq4s3stdwgymwm4zpk6zm7rtxyf91gmtowiarstgue9osy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "wbhfp94iieaccjyfrkej44pdzhkak7zwyuxbyghb4h9sqa3i413y"}) SET u.name = "fafaf", u.bio = "", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "wd94us177uejk78uu3zgfuy1yfzx8mdfhbqwsq7effb7s96pbqwo"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "wfg9rd8qbegqskips78dxzddyq95zz57p31qnobf7myshdxnoh1o"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "wjc7xngyeh3d6h6waka331mm8rn5xasxx86i5ru1obd9y5dzz61o"}) SET u.name = "Foo", u.bio = "foo", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "wnr8986jxopne5786jn88tab6ybygbpykccg9pfg78pxr5g91niy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "wp1m4upxa1rzgturs79u9smc7re7gt8jcafj4ydd8y8rpear1rzy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}) SET u.name = "SHAcollision", u.bio = "", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "x87dnkruxdnd35q7ayzjfhjpqa47gr5a6gdxpb61jymrpngwy1yo"}) SET u.name = "teste", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "xgoium16biftc8z7ecrbwrdokdwr1xphdke4nuaqiutnihdmxmny"}) SET u.name = "Edwin Baumbach", u.bio = "Toties adimpleo tenax comitatus terga.", u.status = "undefined", u.links = "[{\"url\":\"https://sniveling-cyclooxygenase.com/\",\"title\":\"website\"}]";
MERGE (u:User {id: "xguypopohzf1e6h9njbrt94wty6enqqm7m3eqbr677upjdw74uzy"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "xtewe9x8yfuq5sr4tqrk5az47uz4qkt3gxaz5rms6nzugdfo8jao"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}) SET u.name = "Allison Rowe", u.bio = "Vulgaris stultus tenuis articulus.", u.status = "undefined", u.links = "[{\"url\":\"https://sardonic-beef.com/\",\"title\":\"website\"}]";
MERGE (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}) SET u.name = "John Carvalho", u.bio = "Bitcoin Heretic", u.status = "noStatus", u.links = "[{\"title\":\"website\",\"url\":\"https://bitcoin.org\"}]";
MERGE (u:User {id: "y4q8yahtdp6qqu8tzsde83p5zagnzou5cagq9jpt74df67wdt4to"}) SET u.name = "anonymous2", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "y6hjqyajujz61ooecwa1g6fu4s5rj9otka59mir6aeqqbt5xmugo"}) SET u.name = "name", u.bio = "", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "ym1rn4rfjg8857y13uuehc4tw6sfqtzpu881ycdj7iiyo6kqsspy"}) SET u.name = "meanplates", u.bio = "Antishitco.", u.status = "noStatus", u.links = "[{\"title\":\"website\",\"url\":\"https://www.synonym.to\"},{\"title\":\"email\",\"url\":\"ovi@synonym.to\"},{\"title\":\"X\",\"url\":\"https://x.com/ovitrif\"},{\"title\":\"Telegram\",\"url\":\"https://t.me/ovitrif\"}]";
MERGE (u:User {id: "yq9bapr1e981yx8intz88a9oeistye1oe5tucqf9swq59m56497o"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}) SET u.name = "Breizh Ma Bro", u.bio = "blablabla", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "z88bknbxf3q8rxcaehp5strbjgqmyt84yskd4cw7t56rpmakwa8o"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}) SET u.name = "Charlene Medhurst", u.bio = "Degero clamo pauper iusto explicabo defetiscor ademptio pecus.", u.status = "undefined", u.links = "[{\"url\":\"https://clear-literate.biz/\",\"title\":\"website\"}]";
MERGE (u:User {id: "ze86rtgp6x1qdyno4uzp8gexbb887dtemmonoh4j3iisbzitcppo"}) SET u.name = "Matt 2", u.bio = "", u.status = "undefined", u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: "zhf4kyfkt87m4cajm3bjikt4k3jjb3ene7rbxehgi1wz1a77t9co"}) SET u.name = "Jay Uncensored", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "zj7ao1ohiysupjmqbny5gqwiou6afcs1xr5giodsj49aqot7u4jo"}) SET u.name = "anonymous", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "zmztpeny91i48zeswjyrj7sdepe58bc8ru98ymtyfywru3xk31xy"}) SET u.name = "ssss", u.bio = "", u.status = "noStatus", u.links = "[]";
MERGE (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}) SET u.name = "Guy Prosacco", u.bio = "Subvenio adhaero curto ceno creber abundans.", u.status = "undefined", u.links = "[{\"url\":\"https://wee-petal.info/\",\"title\":\"website\"}]";
MERGE (p:Post {id: "0RDV7ABDZDW0"}) SET p.content = "Julian Assange is free", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/0RDV7ABDZDW0", p.createdAt = 1719308315917;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "0RDV7ABDZDW0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RDVFKFBB48G"}) SET p.content = "Hodl! We will implement a mute feature! 🤫", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/0RDVFKFBB48G", p.createdAt = 1719326107982;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "0RDVFKFBB48G"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZDSBY99RAZ00"}), (p2:Post {id: "0RDVFKFBB48G"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "0RDVNHC21YE0"}) SET p.content = "Who are you and why am I following you sir", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/0RDVNHC21YE0", p.createdAt = 1719338851766;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "0RDVNHC21YE0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZDDXBWR520G0"}), (p2:Post {id: "0RDVNHC21YE0"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "0RDXNWJHTCAG"}) SET p.content = "https://media4.giphy.com/media/v1.Y2lkPTc5MGI3NjExZHBieWg3eGpyOGwycTc3aTBkZW5wcTE5czRsbnhuMGoyMml2aGtpaSZlcD12MV9pbnRlcm5hbF9naWZfYnlfaWQmY3Q9Zw/VbQfgkDtYUin6/200.webp", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/0RDXNWJHTCAG", p.createdAt = 1719477042533;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "0RDXNWJHTCAG"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZECXVXHZBE00"}), (p2:Post {id: "0RDXNWJHTCAG"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "0RDXNZC0B0JG"}) SET p.content = "2?", p.uri = "pubky:hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o/pubky.app/posts/0RDXNZC0B0JG", p.createdAt = 1719477230131;
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "0RDXNZC0B0JG"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZEBH4J0K4G00"}), (p2:Post {id: "0RDXNZC0B0JG"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "0RDXQNVBFECG"}) SET p.content = "Hawk.", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/0RDXQNVBFECG", p.createdAt = 1719480886197;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RDXQNVBFECG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RDXQPCD7SRG"}) SET p.content = ":+1:", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/0RDXQPCD7SRG", p.createdAt = 1719480921964;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RDXQPCD7SRG"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZDZ87NZ4H700"}), (p2:Post {id: "0RDXQPCD7SRG"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "0RDXQPTW0WCG"}) SET p.content = "Hai Romania! 🇷🇴", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/0RDXQPTW0WCG", p.createdAt = 1719480952293;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RDXQPTW0WCG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RDXQY80PA1G"}) SET p.content = "When dropzone JS for quick image sharing?", p.uri = "pubky:hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o/pubky.app/posts/0RDXQY80PA1G", p.createdAt = 1719481449624;
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "0RDXQY80PA1G"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RDXR8KKERFG"}) SET p.content = "My posts keep disappearing? ", p.uri = "pubky:hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o/pubky.app/posts/0RDXR8KKERFG", p.createdAt = 1719482145011;
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "0RDXR8KKERFG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RDXRA0GWSZG"}) SET p.content = "I'm being censored!", p.uri = "pubky:hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o/pubky.app/posts/0RDXRA0GWSZG", p.createdAt = 1719482239215;
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "0RDXRA0GWSZG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RDXRNQDCGDG"}) SET p.content = "1st amendment! ", p.uri = "pubky:zhf4kyfkt87m4cajm3bjikt4k3jjb3ene7rbxehgi1wz1a77t9co/pubky.app/posts/0RDXRNQDCGDG", p.createdAt = 1719483025417;
MATCH (u:User {id: "zhf4kyfkt87m4cajm3bjikt4k3jjb3ene7rbxehgi1wz1a77t9co"}), (p:Post {id: "0RDXRNQDCGDG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RDXRNYTJS00"}) SET p.content = "1st amendment", p.uri = "pubky:zhf4kyfkt87m4cajm3bjikt4k3jjb3ene7rbxehgi1wz1a77t9co/pubky.app/posts/0RDXRNYTJS00", p.createdAt = 1719483040962;
MATCH (u:User {id: "zhf4kyfkt87m4cajm3bjikt4k3jjb3ene7rbxehgi1wz1a77t9co"}), (p:Post {id: "0RDXRNYTJS00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RDXRPC82XG0"}) SET p.content = "1st amendment", p.uri = "pubky:zhf4kyfkt87m4cajm3bjikt4k3jjb3ene7rbxehgi1wz1a77t9co/pubky.app/posts/0RDXRPC82XG0", p.createdAt = 1719483069110;
MATCH (u:User {id: "zhf4kyfkt87m4cajm3bjikt4k3jjb3ene7rbxehgi1wz1a77t9co"}), (p:Post {id: "0RDXRPC82XG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RDXX1QHWJDG"}) SET p.content = "Even though it's likely most will use A more frequently, B is still useful for creating a new post, wherever you are on the platform. So, even when looking at your settings, profile, search results, etc, you can start creating a new post.", p.uri = "pubky:4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro/pubky.app/posts/0RDXX1QHWJDG", p.createdAt = 1719492420;
MATCH (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (p:Post {id: "0RDXX1QHWJDG"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZDH5QAWVHT00"}), (p2:Post {id: "0RDXX1QHWJDG"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "0RDY1Y34YPHG"}) SET p.content = "undefined", p.uri = "pubky:4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro/pubky.app/posts/0RDY1Y34YPHG", p.createdAt = 1719502914;
MATCH (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (p:Post {id: "0RDY1Y34YPHG"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2Z1PBYS0F90G0"}), (p2:Post {id: "0RDY1Y34YPHG"}) MERGE (p2)-[:REPOST_OF]->(p1);
MERGE (p:Post {id: "0RE0M0RHQT8G"}) SET p.content = "🧀 1:0 🍝 @Flavio ;)", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/0RE0M0RHQT8G", p.createdAt = 1719679187;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "0RE0M0RHQT8G"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE1V16V88X0"}) SET p.content = "New bio

https://primal.net/e/note1hlf7hw283sdgaj7nw0wa50gf6myupvxtt68zxzcptyx0xsspkxxsxhav76", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/0RE1V16V88X0", p.createdAt = 1719762968;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "0RE1V16V88X0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE39TSPQMWG"}) SET p.content = "a", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/0RE39TSPQMWG", p.createdAt = 1719863470;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "0RE39TSPQMWG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE39VTJCWC0"}) SET p.content = "a", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/0RE39VTJCWC0", p.createdAt = 1719863539;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "0RE39VTJCWC0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE39WFWAQT0"}) SET p.content = "b", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/0RE39WFWAQT0", p.createdAt = 1719863583;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "0RE39WFWAQT0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE3WS2NPCP0"}) SET p.content = "playing with pubky", p.uri = "pubky:77m8jyqqrd41xzu5b67m8z5wuypuatc54gmw5gcax6ae3yeca6wo/pubky.app/posts/0RE3WS2NPCP0", p.createdAt = 1719904157;
MATCH (u:User {id: "77m8jyqqrd41xzu5b67m8z5wuypuatc54gmw5gcax6ae3yeca6wo"}), (p:Post {id: "0RE3WS2NPCP0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE3Z8XXJNH0"}) SET p.content = "Everyone who ever was the job market should watch this 😂
https://youtu.be/YSs5Qp5JbXs?si=1ZaWsyvkR3I_JRAo", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/0RE3Z8XXJNH0", p.createdAt = 1719909515;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RE3Z8XXJNH0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE3ZAZ1JQFG"}) SET p.content = "wen answer?", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/0RE3ZAZ1JQFG", p.createdAt = 1719909652;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RE3ZAZ1JQFG"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2Z9P8AN738C00"}), (p2:Post {id: "0RE3ZAZ1JQFG"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "0RE3ZCC1Z0KG"}) SET p.content = "Bitkit Native Rewrite has officially started. Stay tuned for the real deal, we shall conquer the Lightning UX.

Go Bitkit devs 🚀🚀🚀, nothing can stop us now.", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/0RE3ZCC1Z0KG", p.createdAt = 1719909746;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RE3ZCC1Z0KG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE3ZD46ZB3G"}) SET p.content = "Behold, the nativenning is nigh, and Jay ain't joking around!
https://github.com/synonymdev/bitkit-ios/pull/1/files", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/0RE3ZD46ZB3G", p.createdAt = 1719909797;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RE3ZD46ZB3G"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE3ZFSZTSC0"}) SET p.content = "Plebtest. Y my posts are disappearing? NOOOOOEEEEESSSSSS", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/0RE3ZFSZTSC0", p.createdAt = 1719909977;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RE3ZFSZTSC0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE42ZVNPZG0"}) SET p.content = "antonym 🔥", p.uri = "pubky:uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho/pubky.app/posts/0RE42ZVNPZG0", p.createdAt = 1719917497;
MATCH (u:User {id: "uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho"}), (p:Post {id: "0RE42ZVNPZG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE436QCV2G0"}) SET p.content = "PrivKy", p.uri = "pubky:uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho/pubky.app/posts/0RE436QCV2G0", p.createdAt = 1719917957;
MATCH (u:User {id: "uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho"}), (p:Post {id: "0RE436QCV2G0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "0RE42ZVNPZG0"}), (p2:Post {id: "0RE436QCV2G0"}) MERGE (p2)-[:REPOST_OF]->(p1);
MERGE (p:Post {id: "0RE44D86DRRG"}) SET p.content = "42", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/0RE44D86DRRG", p.createdAt = 1719920543;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "0RE44D86DRRG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4ACZ9Q0HG"}) SET p.content = "a", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/0RE4ACZ9Q0HG", p.createdAt = 1719933409;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "0RE4ACZ9Q0HG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4AF33SW10"}) SET p.content = "d", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/0RE4AF33SW10", p.createdAt = 1719933551;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "0RE4AF33SW10"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4B649XN3G"}) SET p.content = "#hastag #verbose
", p.uri = "pubky:uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho/pubky.app/posts/0RE4B649XN3G", p.createdAt = 1719935097;
MATCH (u:User {id: "uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho"}), (p:Post {id: "0RE4B649XN3G"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4B9R858RG"}) SET p.content = "Help!", p.uri = "pubky:hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o/pubky.app/posts/0RE4B9R858RG", p.createdAt = 1719935340;
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "0RE4B9R858RG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4CYTZ7YN0"}) SET p.content = "test 3", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/0RE4CYTZ7YN0", p.createdAt = 1719938903;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "0RE4CYTZ7YN0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4CZ42R3CG"}) SET p.content = "test 4", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/0RE4CZ42R3CG", p.createdAt = 1719938922;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "0RE4CZ42R3CG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4CZY7FFZG"}) SET p.content = "test 5", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/0RE4CZY7FFZG", p.createdAt = 1719938977;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "0RE4CZY7FFZG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4D7VRD83G"}) SET p.content = "test", p.uri = "pubky:h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy/pubky.app/posts/0RE4D7VRD83G", p.createdAt = 1719939509;
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "0RE4D7VRD83G"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4DDJ3XHKG"}) SET p.content = "test 3", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/0RE4DDJ3XHKG", p.createdAt = 1719939891;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "0RE4DDJ3XHKG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4DE8TZ4Q0"}) SET p.content = "test", p.uri = "pubky:h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy/pubky.app/posts/0RE4DE8TZ4Q0", p.createdAt = 1719939939;
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "0RE4DE8TZ4Q0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4DF1KXMQ0"}) SET p.content = "test", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/0RE4DF1KXMQ0", p.createdAt = 1719939991;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "0RE4DF1KXMQ0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4DF6HRD20"}) SET p.content = "new test post", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/0RE4DF6HRD20", p.createdAt = 1719940001;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "0RE4DF6HRD20"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4DGDHP0P0"}) SET p.content = "hghjghjgj", p.uri = "pubky:h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy/pubky.app/posts/0RE4DGDHP0P0", p.createdAt = 1719940083;
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "0RE4DGDHP0P0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4DNDBXSD0"}) SET p.content = "posting on production", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/0RE4DNDBXSD0", p.createdAt = 1719940418;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "0RE4DNDBXSD0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE4DYC0BVN0"}) SET p.content = "test", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/0RE4DYC0BVN0", p.createdAt = 1719941019;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "0RE4DYC0BVN0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE51NMRZAQG"}) SET p.content = "hey hey heyyy!!! 🎤", p.uri = "pubky:7oq5wj1adxk1u94ojh6eknwj3b4z88zcbb51dbam5zn7zeqnzoio/pubky.app/posts/0RE51NMRZAQG", p.createdAt = 1719983383;
MATCH (u:User {id: "7oq5wj1adxk1u94ojh6eknwj3b4z88zcbb51dbam5zn7zeqnzoio"}), (p:Post {id: "0RE51NMRZAQG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE51RE8RBYG"}) SET p.content = "whaatsssUUppp", p.uri = "pubky:uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho/pubky.app/posts/0RE51RE8RBYG", p.createdAt = 1719983571;
MATCH (u:User {id: "uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho"}), (p:Post {id: "0RE51RE8RBYG"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "0RE51NMRZAQG"}), (p2:Post {id: "0RE51RE8RBYG"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "0RE51S21S6PG"}) SET p.content = "hello hellooo", p.uri = "pubky:uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho/pubky.app/posts/0RE51S21S6PG", p.createdAt = 1719983612;
MATCH (u:User {id: "uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho"}), (p:Post {id: "0RE51S21S6PG"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "0RE51NMRZAQG"}), (p2:Post {id: "0RE51S21S6PG"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "0RE5A8J4JBC0"}) SET p.content = "warming the events", p.uri = "pubky:uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho/pubky.app/posts/0RE5A8J4JBC0", p.createdAt = 1720001832;
MATCH (u:User {id: "uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho"}), (p:Post {id: "0RE5A8J4JBC0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "0RE51NMRZAQG"}), (p2:Post {id: "0RE5A8J4JBC0"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "0RE6H5Q7YEA0"}) SET p.content = "True love.", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/0RE6H5Q7YEA0", p.createdAt = 1720085394;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RE6H5Q7YEA0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZF7PFV56HRG0"}), (p2:Post {id: "0RE6H5Q7YEA0"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "0RE6HA9J8DT0"}) SET p.content = "It is now possible to require eSignature in Google Docs.

GG DocuSign.
You had a good run.

#rip", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/0RE6HA9J8DT0", p.createdAt = 1720085701;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RE6HA9J8DT0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE6HCGAKWY0"}) SET p.content = "Fire in the hole!", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/0RE6HCGAKWY0", p.createdAt = 1720085849;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RE6HCGAKWY0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZECXVXHZBE00"}), (p2:Post {id: "0RE6HCGAKWY0"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "0RE6HD1Q5HW0"}) SET p.content = "Test", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/0RE6HD1Q5HW0", p.createdAt = 1720085885;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "0RE6HD1Q5HW0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE6HF3ZQCA0"}) SET p.content = "@Flavio Moceri

When CMD+ENTER sir?", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/0RE6HF3ZQCA0", p.createdAt = 1720086024;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RE6HF3ZQCA0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE6JBGJ2W8G"}) SET p.content = "BREAKING: Proton Docs has LANDED 🚀🚀🚀 

https://proton.me/blog/docs-proton-drive", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/0RE6JBGJ2W8G", p.createdAt = 1720087930;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RE6JBGJ2W8G"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "0RE72VE7MCRG"}) SET p.content = "Dammit Zuck is cool AF!

https://x.com/greg16676935420/status/1808906173598629926", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/0RE72VE7MCRG", p.createdAt = 1720123358;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RE72VE7MCRG"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N38NX16P00"}) SET p.content = "First post", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z1N38NX16P00", p.createdAt = 1712302042729;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1N38NX16P00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N541P346G0"}) SET p.content = "Hello world!", p.uri = "pubky:hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o/pubky.app/posts/2Z1N541P346G0", p.createdAt = 1712303062667;
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1N541P346G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBER6300"}) SET p.content = "Utpote stultus copiose delego concido aegrus.", p.uri = "pubky:kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o/pubky.app/posts/2Z1N8QBER6300", p.createdAt = 1712305043597;
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBER6300"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBERAVG0"}) SET p.content = "Volup basium exercitationem.", p.uri = "pubky:j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy/pubky.app/posts/2Z1N8QBERAVG0", p.createdAt = 1712305043597;
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBERAVG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBERD800"}) SET p.content = "Ascit validus arguo tondeo comptus campana solium dolorum.", p.uri = "pubky:oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo/pubky.app/posts/2Z1N8QBERD800", p.createdAt = 1712305043597;
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBERD800"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBERF700"}) SET p.content = "Confero demo cupiditate suscipio labore sol attonbitus.", p.uri = "pubky:78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y/pubky.app/posts/2Z1N8QBERF700", p.createdAt = 1712305043597;
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBERF700"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBERHB00"}) SET p.content = "Deprecator temptatio atrocitas auctus sublime quis valde alter spargo uredo.", p.uri = "pubky:pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo/pubky.app/posts/2Z1N8QBERHB00", p.createdAt = 1712305043597;
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERHB00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBERJQ00"}) SET p.content = "Vero argentum aedificium.", p.uri = "pubky:pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo/pubky.app/posts/2Z1N8QBERJQ00", p.createdAt = 1712305043597;
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERJQ00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBERMMG0"}) SET p.content = "Suadeo vulnus utilis vigor pectus reiciendis basium velut cultura.", p.uri = "pubky:ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y/pubky.app/posts/2Z1N8QBERMMG0", p.createdAt = 1712305043598;
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBERMMG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBERNYG0"}) SET p.content = "Earum agnosco cena.", p.uri = "pubky:ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go/pubky.app/posts/2Z1N8QBERNYG0", p.createdAt = 1712305043598;
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBERNYG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBERQ9G0"}) SET p.content = "Aperio adipiscor supra.", p.uri = "pubky:7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so/pubky.app/posts/2Z1N8QBERQ9G0", p.createdAt = 1712305043598;
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBERQ9G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBERS9G0"}) SET p.content = "Cena averto vinculum aestivus video culpo auctor asper cultura eligendi.", p.uri = "pubky:pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo/pubky.app/posts/2Z1N8QBERS9G0", p.createdAt = 1712305043598;
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERS9G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBERTQG0"}) SET p.content = "Veritatis confero quos acies.", p.uri = "pubky:7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so/pubky.app/posts/2Z1N8QBERTQG0", p.createdAt = 1712305043598;
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBERTQG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBERWS00"}) SET p.content = "Armarium absum tamdiu tibi impedit tutis succurro cras acidus.", p.uri = "pubky:ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go/pubky.app/posts/2Z1N8QBERWS00", p.createdAt = 1712305043598;
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBERWS00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBESAF00"}) SET p.content = "Arx adstringo curatio subiungo tenus umerus.", p.uri = "pubky:78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y/pubky.app/posts/2Z1N8QBESAF00", p.createdAt = 1712305043598;
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESAF00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBESBWG0"}) SET p.content = "Curiositas numquam tui.", p.uri = "pubky:oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo/pubky.app/posts/2Z1N8QBESBWG0", p.createdAt = 1712305043598;
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESBWG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBESDD00"}) SET p.content = "Aggredior tonsor cicuta sol uter occaecati.", p.uri = "pubky:ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y/pubky.app/posts/2Z1N8QBESDD00", p.createdAt = 1712305043598;
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBESDD00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBESER00"}) SET p.content = "Tenax calamitas cupiditate adfectus.", p.uri = "pubky:oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo/pubky.app/posts/2Z1N8QBESER00", p.createdAt = 1712305043598;
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESER00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBESGGG0"}) SET p.content = "Circumvenio abscido omnis deserunt.", p.uri = "pubky:ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y/pubky.app/posts/2Z1N8QBESGGG0", p.createdAt = 1712305043598;
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBESGGG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBESKYG0"}) SET p.content = "Utrum testimonium traho absconditus aegrotatio reiciendis.", p.uri = "pubky:pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco/pubky.app/posts/2Z1N8QBESKYG0", p.createdAt = 1712305043598;
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBESKYG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBESNE00"}) SET p.content = "Solium validus terga.", p.uri = "pubky:78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y/pubky.app/posts/2Z1N8QBESNE00", p.createdAt = 1712305043598;
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESNE00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBESPR00"}) SET p.content = "Theologus vos speculum vigilo.", p.uri = "pubky:pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo/pubky.app/posts/2Z1N8QBESPR00", p.createdAt = 1712305043598;
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBESPR00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBESRFG0"}) SET p.content = "Calculus quidem temeritas supplanto ultio acer cohors terra facere utor.", p.uri = "pubky:7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so/pubky.app/posts/2Z1N8QBESRFG0", p.createdAt = 1712305043598;
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBESRFG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBEST8G0"}) SET p.content = "Studio virga cunabula abbas coniuratio conor clibanus audacia confido desidero.", p.uri = "pubky:pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo/pubky.app/posts/2Z1N8QBEST8G0", p.createdAt = 1712305043598;
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBEST8G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBESVC00"}) SET p.content = "Velum adeptio acsi.", p.uri = "pubky:78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y/pubky.app/posts/2Z1N8QBESVC00", p.createdAt = 1712305043598;
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESVC00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBESWP00"}) SET p.content = "Quae denuo acervus circumvenio adduco odio.", p.uri = "pubky:kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o/pubky.app/posts/2Z1N8QBESWP00", p.createdAt = 1712305043598;
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBESWP00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBESY900"}) SET p.content = "Cauda non defero tyrannus solio censura deficio sed campana.", p.uri = "pubky:uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy/pubky.app/posts/2Z1N8QBESY900", p.createdAt = 1712305043598;
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBESY900"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBESZCG0"}) SET p.content = "Surculus cado supra.", p.uri = "pubky:pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco/pubky.app/posts/2Z1N8QBESZCG0", p.createdAt = 1712305043598;
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBESZCG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBET1P00"}) SET p.content = "Cunae vulnero clam voluptatem calculus uterque.", p.uri = "pubky:78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y/pubky.app/posts/2Z1N8QBET1P00", p.createdAt = 1712305043598;
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET1P00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBET4BG0"}) SET p.content = "Deporto tres campana decretum subvenio derelinquo titulus aeger canto aperte.", p.uri = "pubky:78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y/pubky.app/posts/2Z1N8QBET4BG0", p.createdAt = 1712305043598;
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET4BG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBET5GG0"}) SET p.content = "Angustus facilis defetiscor.", p.uri = "pubky:ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y/pubky.app/posts/2Z1N8QBET5GG0", p.createdAt = 1712305043598;
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBET5GG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBET6PG0"}) SET p.content = "Fugit terebro terreo.", p.uri = "pubky:7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so/pubky.app/posts/2Z1N8QBET6PG0", p.createdAt = 1712305043598;
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBET6PG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBET89G0"}) SET p.content = "Carcer nostrum volutabrum comprehendo quo aurum vel tabgo sol.", p.uri = "pubky:ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go/pubky.app/posts/2Z1N8QBET89G0", p.createdAt = 1712305043598;
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET89G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBET9MG0"}) SET p.content = "Thalassinus degenero viriliter adstringo caries somnus.", p.uri = "pubky:ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go/pubky.app/posts/2Z1N8QBET9MG0", p.createdAt = 1712305043598;
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET9MG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETAWG0"}) SET p.content = "Tamisium ulterius accendo averto denique.", p.uri = "pubky:ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go/pubky.app/posts/2Z1N8QBETAWG0", p.createdAt = 1712305043598;
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBETAWG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETC900"}) SET p.content = "Est tonsor supra aliquid varius eos.", p.uri = "pubky:pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo/pubky.app/posts/2Z1N8QBETC900", p.createdAt = 1712305043598;
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBETC900"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETDS00"}) SET p.content = "Bardus vinculum suus ventito tepidus callide comis decipio.", p.uri = "pubky:ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y/pubky.app/posts/2Z1N8QBETDS00", p.createdAt = 1712305043598;
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETDS00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETEYG0"}) SET p.content = "Carpo verbum ocer thorax.", p.uri = "pubky:7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so/pubky.app/posts/2Z1N8QBETEYG0", p.createdAt = 1712305043598;
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETEYG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETG5G0"}) SET p.content = "Quia pectus alter cupiditas solus.", p.uri = "pubky:kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o/pubky.app/posts/2Z1N8QBETG5G0", p.createdAt = 1712305043599;
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETG5G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETHK00"}) SET p.content = "Tandem hic audentia beneficium absque celebrer aurum.", p.uri = "pubky:kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o/pubky.app/posts/2Z1N8QBETHK00", p.createdAt = 1712305043599;
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETHK00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETJMG0"}) SET p.content = "Aperte sequi bonus.", p.uri = "pubky:pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco/pubky.app/posts/2Z1N8QBETJMG0", p.createdAt = 1712305043599;
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBETJMG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETKR00"}) SET p.content = "Crebro argumentum accendo beneficium.", p.uri = "pubky:kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o/pubky.app/posts/2Z1N8QBETKR00", p.createdAt = 1712305043599;
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETKR00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETN1G0"}) SET p.content = "Acidus depromo sollicito adsuesco coerceo ulciscor.", p.uri = "pubky:ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y/pubky.app/posts/2Z1N8QBETN1G0", p.createdAt = 1712305043599;
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETN1G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETP6G0"}) SET p.content = "Degusto tubineus agnitio sapiente.", p.uri = "pubky:7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so/pubky.app/posts/2Z1N8QBETP6G0", p.createdAt = 1712305043599;
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETP6G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETQK00"}) SET p.content = "Derelinquo tero celebrer demitto thema laboriosam volubilis.", p.uri = "pubky:j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy/pubky.app/posts/2Z1N8QBETQK00", p.createdAt = 1712305043599;
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBETQK00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETRYG0"}) SET p.content = "Pax cenaculum conspergo defungo spectaculum.", p.uri = "pubky:78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y/pubky.app/posts/2Z1N8QBETRYG0", p.createdAt = 1712305043599;
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETRYG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETTM00"}) SET p.content = "Dolor verus annus demens traho auxilium dedecor temeritas currus demens.", p.uri = "pubky:78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y/pubky.app/posts/2Z1N8QBETTM00", p.createdAt = 1712305043599;
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETTM00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETW700"}) SET p.content = "Solitudo constans vulnero comes cruciamentum doloribus quia cursim vinculum.", p.uri = "pubky:kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o/pubky.app/posts/2Z1N8QBETW700", p.createdAt = 1712305043599;
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETW700"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETXT00"}) SET p.content = "Cupiditas magnam cursim umerus amaritudo curtus temptatio suffoco triumphus.", p.uri = "pubky:78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y/pubky.app/posts/2Z1N8QBETXT00", p.createdAt = 1712305043599;
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETXT00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBETZ1G0"}) SET p.content = "Nostrum viridis fuga perferendis abeo.", p.uri = "pubky:uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy/pubky.app/posts/2Z1N8QBETZ1G0", p.createdAt = 1712305043599;
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBETZ1G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBEV03G0"}) SET p.content = "Tener distinctio similique.", p.uri = "pubky:uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy/pubky.app/posts/2Z1N8QBEV03G0", p.createdAt = 1712305043599;
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBEV03G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N8QBEV1D00"}) SET p.content = "Caveo commemoro creta delicate facere speculum.", p.uri = "pubky:kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o/pubky.app/posts/2Z1N8QBEV1D00", p.createdAt = 1712305043599;
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBEV1D00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56VY8G0"}) SET p.content = "Cunae apparatus amita commemoro tripudio admiratio suffragium.", p.uri = "pubky:zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy/pubky.app/posts/2Z1N9M56VY8G0", p.createdAt = 1712305538460;
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56VY8G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56W1EG0"}) SET p.content = "Vapulus atqui dolorum unde ater.", p.uri = "pubky:me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy/pubky.app/posts/2Z1N9M56W1EG0", p.createdAt = 1712305538460;
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W1EG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56W2DG0"}) SET p.content = "Truculenter turbo calcar alo constans quas aegre angelus.", p.uri = "pubky:zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy/pubky.app/posts/2Z1N9M56W2DG0", p.createdAt = 1712305538460;
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W2DG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56W3CG0"}) SET p.content = "Cibus vobis capitulus stillicidium admoveo administratio.", p.uri = "pubky:zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy/pubky.app/posts/2Z1N9M56W3CG0", p.createdAt = 1712305538460;
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56W3CG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56W3ZG0"}) SET p.content = "Maxime calco celebrer.", p.uri = "pubky:5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy/pubky.app/posts/2Z1N9M56W3ZG0", p.createdAt = 1712305538460;
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56W3ZG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56W4VG0"}) SET p.content = "Creo adeo iusto testimonium aegre.", p.uri = "pubky:me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy/pubky.app/posts/2Z1N9M56W4VG0", p.createdAt = 1712305538460;
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W4VG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56W5N00"}) SET p.content = "Bibo suasoria adhuc concedo spero crux contego cotidie amplexus.", p.uri = "pubky:ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy/pubky.app/posts/2Z1N9M56W5N00", p.createdAt = 1712305538460;
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56W5N00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56W6MG0"}) SET p.content = "Vulgo creo tertius vulgaris iure molestias.", p.uri = "pubky:ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy/pubky.app/posts/2Z1N9M56W6MG0", p.createdAt = 1712305538460;
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56W6MG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56W7M00"}) SET p.content = "Cupio votum desino quisquam compello cognatus.", p.uri = "pubky:me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy/pubky.app/posts/2Z1N9M56W7M00", p.createdAt = 1712305538460;
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W7M00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56W8D00"}) SET p.content = "Cattus adamo aperiam itaque sed consuasor socius deputo complectus.", p.uri = "pubky:zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy/pubky.app/posts/2Z1N9M56W8D00", p.createdAt = 1712305538460;
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W8D00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56W9K00"}) SET p.content = "Infit sponte advoco totam demitto.", p.uri = "pubky:xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to/pubky.app/posts/2Z1N9M56W9K00", p.createdAt = 1712305538460;
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56W9K00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WA600"}) SET p.content = "Teneo cornu benevolentia volup.", p.uri = "pubky:bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo/pubky.app/posts/2Z1N9M56WA600", p.createdAt = 1712305538460;
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WA600"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WG1G0"}) SET p.content = "Avaritia adulatio sol amor arceo.", p.uri = "pubky:bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo/pubky.app/posts/2Z1N9M56WG1G0", p.createdAt = 1712305538460;
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WG1G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WGYG0"}) SET p.content = "Non colo esse cultura compono aptus auditor.", p.uri = "pubky:9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o/pubky.app/posts/2Z1N9M56WGYG0", p.createdAt = 1712305538460;
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56WGYG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WHQG0"}) SET p.content = "Surgo contra curatio atrox.", p.uri = "pubky:zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy/pubky.app/posts/2Z1N9M56WHQG0", p.createdAt = 1712305538460;
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WHQG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WJKG0"}) SET p.content = "Acerbitas amplitudo aliqua stips sum debeo acsi.", p.uri = "pubky:xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to/pubky.app/posts/2Z1N9M56WJKG0", p.createdAt = 1712305538460;
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WJKG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WKE00"}) SET p.content = "Theologus callide atavus dedecor terreo.", p.uri = "pubky:xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to/pubky.app/posts/2Z1N9M56WKE00", p.createdAt = 1712305538460;
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WKE00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WN8G0"}) SET p.content = "Sumptus vobis viscus circumvenio.", p.uri = "pubky:5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy/pubky.app/posts/2Z1N9M56WN8G0", p.createdAt = 1712305538460;
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WN8G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WNX00"}) SET p.content = "Soleo quas quod.", p.uri = "pubky:xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to/pubky.app/posts/2Z1N9M56WNX00", p.createdAt = 1712305538460;
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WNX00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WPS00"}) SET p.content = "Cedo amet cito adstringo absconditus taedium una desolo.", p.uri = "pubky:xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to/pubky.app/posts/2Z1N9M56WPS00", p.createdAt = 1712305538460;
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WPS00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WQFG0"}) SET p.content = "Curriculum terminatio callide turbo amplitudo tabella vulnus voco modi.", p.uri = "pubky:ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy/pubky.app/posts/2Z1N9M56WQFG0", p.createdAt = 1712305538460;
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56WQFG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WREG0"}) SET p.content = "Verbum caelestis vivo ceno summa audeo ustulo ait tres.", p.uri = "pubky:zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy/pubky.app/posts/2Z1N9M56WREG0", p.createdAt = 1712305538460;
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WREG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WSCG0"}) SET p.content = "Volo coruscus copia voluptatum degero repudiandae abeo statua bonus suscipio.", p.uri = "pubky:bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo/pubky.app/posts/2Z1N9M56WSCG0", p.createdAt = 1712305538460;
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WSCG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WT600"}) SET p.content = "Ullam comis conor tyrannus deduco demo color validus.", p.uri = "pubky:me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy/pubky.app/posts/2Z1N9M56WT600", p.createdAt = 1712305538460;
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56WT600"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WTQG0"}) SET p.content = "Vociferor tracto claustrum cito.", p.uri = "pubky:zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy/pubky.app/posts/2Z1N9M56WTQG0", p.createdAt = 1712305538460;
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56WTQG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WVEG0"}) SET p.content = "Stabilis appello cimentarius coma carmen damnatio.", p.uri = "pubky:5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy/pubky.app/posts/2Z1N9M56WVEG0", p.createdAt = 1712305538460;
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WVEG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WW600"}) SET p.content = "Cui tibi cumque inflammatio sub tandem titulus caritas tactus veritatis.", p.uri = "pubky:bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo/pubky.app/posts/2Z1N9M56WW600", p.createdAt = 1712305538461;
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WW600"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WX200"}) SET p.content = "Spectaculum tolero at harum complectus contabesco a subseco caveo alveus.", p.uri = "pubky:9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o/pubky.app/posts/2Z1N9M56WX200", p.createdAt = 1712305538461;
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56WX200"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WXJ00"}) SET p.content = "Caecus absconditus rerum attero.", p.uri = "pubky:ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy/pubky.app/posts/2Z1N9M56WXJ00", p.createdAt = 1712305538461;
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56WXJ00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WY1G0"}) SET p.content = "Confero subito bonus.", p.uri = "pubky:5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy/pubky.app/posts/2Z1N9M56WY1G0", p.createdAt = 1712305538461;
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WY1G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WYXG0"}) SET p.content = "Verecundia strenuus soluta sed decretum blanditiis texo spiritus.", p.uri = "pubky:xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to/pubky.app/posts/2Z1N9M56WYXG0", p.createdAt = 1712305538461;
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WYXG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56WZK00"}) SET p.content = "Vorax maiores reprehenderit absconditus acquiro autus doloremque varius celo.", p.uri = "pubky:xgoium16biftc8z7ecrbwrdokdwr1xphdke4nuaqiutnihdmxmny/pubky.app/posts/2Z1N9M56WZK00", p.createdAt = 1712305538461;
MATCH (u:User {id: "xgoium16biftc8z7ecrbwrdokdwr1xphdke4nuaqiutnihdmxmny"}), (p:Post {id: "2Z1N9M56WZK00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X0D00"}) SET p.content = "Caute circumvenio bellicus volo calcar modi supra solum adversus.", p.uri = "pubky:me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy/pubky.app/posts/2Z1N9M56X0D00", p.createdAt = 1712305538461;
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X0D00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X0XG0"}) SET p.content = "Fugiat defluo patior universe.", p.uri = "pubky:zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy/pubky.app/posts/2Z1N9M56X0XG0", p.createdAt = 1712305538461;
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X0XG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X1SG0"}) SET p.content = "Voluptatum torqueo tracto velum sophismata creptio autem incidunt basium.", p.uri = "pubky:xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to/pubky.app/posts/2Z1N9M56X1SG0", p.createdAt = 1712305538461;
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X1SG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X2C00"}) SET p.content = "Quod tollo acceptus pel benevolentia tristis.", p.uri = "pubky:7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso/pubky.app/posts/2Z1N9M56X2C00", p.createdAt = 1712305538461;
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X2C00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X2ZG0"}) SET p.content = "Ulterius molestias aperiam molestiae.", p.uri = "pubky:9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o/pubky.app/posts/2Z1N9M56X2ZG0", p.createdAt = 1712305538461;
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56X2ZG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X3N00"}) SET p.content = "Comitatus velociter vilicus synagoga thalassinus distinctio inflammatio quis aetas.", p.uri = "pubky:zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy/pubky.app/posts/2Z1N9M56X3N00", p.createdAt = 1712305538461;
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56X3N00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X4EG0"}) SET p.content = "Attonbitus ultra autus calculus viduo corporis.", p.uri = "pubky:7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso/pubky.app/posts/2Z1N9M56X4EG0", p.createdAt = 1712305538461;
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X4EG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X55G0"}) SET p.content = "Damno chirographum suspendo spectaculum combibo tricesimus tabgo cura truculenter aut.", p.uri = "pubky:ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy/pubky.app/posts/2Z1N9M56X55G0", p.createdAt = 1712305538461;
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56X55G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X5VG0"}) SET p.content = "Tabgo solus thymum carcer verus.", p.uri = "pubky:xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to/pubky.app/posts/2Z1N9M56X5VG0", p.createdAt = 1712305538461;
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X5VG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X6JG0"}) SET p.content = "Celo defaeco thesis perspiciatis sono claudeo adeo minus cras caecus.", p.uri = "pubky:me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy/pubky.app/posts/2Z1N9M56X6JG0", p.createdAt = 1712305538461;
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X6JG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X7DG0"}) SET p.content = "Tempora amicitia decor curis cuppedia decretum amissio thalassinus.", p.uri = "pubky:zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy/pubky.app/posts/2Z1N9M56X7DG0", p.createdAt = 1712305538461;
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X7DG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X7Y00"}) SET p.content = "Velit dolore non torqueo.", p.uri = "pubky:5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy/pubky.app/posts/2Z1N9M56X7Y00", p.createdAt = 1712305538461;
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56X7Y00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X8C00"}) SET p.content = "Tui vigor veritatis.", p.uri = "pubky:5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy/pubky.app/posts/2Z1N9M56X8C00", p.createdAt = 1712305538461;
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56X8C00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X9000"}) SET p.content = "Vesper aptus ancilla adicio.", p.uri = "pubky:9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o/pubky.app/posts/2Z1N9M56X9000", p.createdAt = 1712305538461;
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56X9000"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56X9M00"}) SET p.content = "Ascit comminor cimentarius delibero deporto appono provident usus.", p.uri = "pubky:zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy/pubky.app/posts/2Z1N9M56X9M00", p.createdAt = 1712305538461;
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56X9M00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56XA1G0"}) SET p.content = "Quia complectus damno.", p.uri = "pubky:zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy/pubky.app/posts/2Z1N9M56XA1G0", p.createdAt = 1712305538461;
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56XA1G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56XB2G0"}) SET p.content = "Vesica undique tollo tempora triumphus acsi trepide socius.", p.uri = "pubky:7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso/pubky.app/posts/2Z1N9M56XB2G0", p.createdAt = 1712305538461;
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56XB2G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1N9M56XBP00"}) SET p.content = "Excepturi cicuta vulticulus ciminatio confero arguo ducimus voluptas.", p.uri = "pubky:bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo/pubky.app/posts/2Z1N9M56XBP00", p.createdAt = 1712305538461;
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56XBP00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1NB44D42MG0"}) SET p.content = "hello world", p.uri = "pubky:ze86rtgp6x1qdyno4uzp8gexbb887dtemmonoh4j3iisbzitcppo/pubky.app/posts/2Z1NB44D42MG0", p.createdAt = 1712306362662;
MATCH (u:User {id: "ze86rtgp6x1qdyno4uzp8gexbb887dtemmonoh4j3iisbzitcppo"}), (p:Post {id: "2Z1NB44D42MG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1NB53BF82G0"}) SET p.content = "kljdfgkjdflgjfl", p.uri = "pubky:omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy/pubky.app/posts/2Z1NB53BF82G0", p.createdAt = 1712306379277;
MATCH (u:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}), (p:Post {id: "2Z1NB53BF82G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1NCPSDTW400"}) SET p.content = "P2P EVERYTHING 🍐", p.uri = "pubky:9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y/pubky.app/posts/2Z1NCPSDTW400", p.createdAt = 1712307232942;
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NCPSDTW400"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1NCTJJXZTG0"}) SET p.content = "hello world", p.uri = "pubky:end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso/pubky.app/posts/2Z1NCTJJXZTG0", p.createdAt = 1712307297988;
MATCH (u:User {id: "end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso"}), (p:Post {id: "2Z1NCTJJXZTG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1NDENYVN5G0"}) SET p.content = "first rule about Pubky is - you DO TALK about Pubky", p.uri = "pubky:ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno/pubky.app/posts/2Z1NDENYVN5G0", p.createdAt = 1712307643397;
MATCH (u:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}), (p:Post {id: "2Z1NDENYVN5G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1NDJYBBBEG0"}) SET p.content = "GM", p.uri = "pubky:614ohi1318w3hts3bw89x3t4y8ctychdx6xm68prm478xrir1k8o/pubky.app/posts/2Z1NDJYBBBEG0", p.createdAt = 1712307716621;
MATCH (u:User {id: "614ohi1318w3hts3bw89x3t4y8ctychdx6xm68prm478xrir1k8o"}), (p:Post {id: "2Z1NDJYBBBEG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1NDNKKXYV00"}) SET p.content = "running pukey", p.uri = "pubky:wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo/pubky.app/posts/2Z1NDNKKXYV00", p.createdAt = 1712307762399;
MATCH (u:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}), (p:Post {id: "2Z1NDNKKXYV00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1NGNZNVCM00"}) SET p.content = "gm!", p.uri = "pubky:phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco/pubky.app/posts/2Z1NGNZNVCM00", p.createdAt = 1712309418141;
MATCH (u:User {id: "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco"}), (p:Post {id: "2Z1NGNZNVCM00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1NHJGEGAFG0"}) SET p.content = "This is fine", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z1NHJGEGAFG0", p.createdAt = 1712309908181;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1NHJGEGAFG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1NJ21ZTWW00"}) SET p.content = "🖖", p.uri = "pubky:ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro/pubky.app/posts/2Z1NJ21ZTWW00", p.createdAt = 1712310175296;
MATCH (u:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}), (p:Post {id: "2Z1NJ21ZTWW00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1NJPW2QHGG0"}) SET p.content = "Running #Pubky ", p.uri = "pubky:4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro/pubky.app/posts/2Z1NJPW2QHGG0", p.createdAt = 1712310532901;
MATCH (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (p:Post {id: "2Z1NJPW2QHGG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1NP1EQ2PA00"}) SET p.content = "+1 for TOMATO 🍅", p.uri = "pubky:hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o/pubky.app/posts/2Z1NP1EQ2PA00", p.createdAt = 1712312363972;
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1NP1EQ2PA00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1NRDN245400"}) SET p.content = "+1 for pubky-core

Tag Poll 👇", p.uri = "pubky:9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y/pubky.app/posts/2Z1NRDN245400", p.createdAt = 1712313673049;
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NRDN245400"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1P3VV14ZD00"}) SET p.content = "hey what's #hup", p.uri = "pubky:614ohi1318w3hts3bw89x3t4y8ctychdx6xm68prm478xrir1k8o/pubky.app/posts/2Z1P3VV14ZD00", p.createdAt = 1712319964086;
MATCH (u:User {id: "614ohi1318w3hts3bw89x3t4y8ctychdx6xm68prm478xrir1k8o"}), (p:Post {id: "2Z1P3VV14ZD00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1P61QPX7Q00"}) SET p.content = "Hello world!", p.uri = "pubky:omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy/pubky.app/posts/2Z1P61QPX7Q00", p.createdAt = 1712321164894;
MATCH (u:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}), (p:Post {id: "2Z1P61QPX7Q00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1P68V42JJ00"}) SET p.content = "Any bugs are your fault", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z1P68V42JJ00", p.createdAt = 1712321286985;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1P68V42JJ00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1P778B2G800"}) SET p.content = "Congratulations on the progress!", p.uri = "pubky:y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pubky.app/posts/2Z1P778B2G800", p.createdAt = 1712321809477;
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z1P778B2G800"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1P7H9Y8QV00"}) SET p.content = "Hello world!", p.uri = "pubky:hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o/pubky.app/posts/2Z1P7H9Y8QV00", p.createdAt = 1712321982135;
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1P7H9Y8QV00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1P7HCM7WYG0"}) SET p.content = "Hodl", p.uri = "pubky:sse599pgxr9ahwrrns7yz3dpd118wjjpiisscrh4qnrgj4334iuo/pubky.app/posts/2Z1P7HCM7WYG0", p.createdAt = 1712321983577;
MATCH (u:User {id: "sse599pgxr9ahwrrns7yz3dpd118wjjpiisscrh4qnrgj4334iuo"}), (p:Post {id: "2Z1P7HCM7WYG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1P8ET4Z7T00"}) SET p.content = "Matt should create a Synonym Pubky account", p.uri = "pubky:9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y/pubky.app/posts/2Z1P8ET4Z7T00", p.createdAt = 1712322489054;
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1P8ET4Z7T00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1PBYS0F90G0"}) SET p.content = "hashtag vs tag", p.uri = "pubky:hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o/pubky.app/posts/2Z1PBYS0F90G0", p.createdAt = 1712324412587;
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1PBYS0F90G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z1PP0D914200"}) SET p.content = "FUD", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z1PP0D914200", p.createdAt = 1712329938206;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1PP0D914200"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z23VQHKR6YG0"}) SET p.content = "Working!", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z23VQHKR6YG0", p.createdAt = 1712561782868;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z23VQHKR6YG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z23W99C75EG0"}) SET p.content = "Roger Ver was wright.", p.uri = "pubky:y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pubky.app/posts/2Z23W99C75EG0", p.createdAt = 1712562087684;
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z23W99C75EG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z245TC9K7200"}) SET p.content = "The Web, long centralized, must decentralize; Long decentralized, must centralize.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z245TC9K7200", p.createdAt = 1712567329111;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z245TC9K7200"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z25B7X03Q700"}) SET p.content = "i'm in", p.uri = "pubky:fy3ekk5mfm8nje69nwqb7yhou79kjhm41qp35px8o6zcbqc51k5y/pubky.app/posts/2Z25B7X03Q700", p.createdAt = 1712587902382;
MATCH (u:User {id: "fy3ekk5mfm8nje69nwqb7yhou79kjhm41qp35px8o6zcbqc51k5y"}), (p:Post {id: "2Z25B7X03Q700"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z29ACD50BQ00"}) SET p.content = "First weekly dev call after Pubky demo!", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z29ACD50BQ00", p.createdAt = 1712657798761;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z29ACD50BQ00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z2R1H784JD00"}) SET p.content = "Posting from my hosted frontend hooked to pkarr.org relay and pk:4unkz8qto4xec6jhw9mie9oepgcurirebdx8axyq3o36fanooxxy server.", p.uri = "pubky:wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo/pubky.app/posts/2Z2R1H784JD00", p.createdAt = 1712916816481;
MATCH (u:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}), (p:Post {id: "2Z2R1H784JD00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z3D64GPYF4G0"}) SET p.content = "This website is growing on me.

I guess the main use case for Twitter was shouting to the void!

But if \"reach\" isn't that important, might as well focus on censorship resistance. Maybe even cultivate engagement within close circles.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z3D64GPYF4G0", p.createdAt = 1713288782909;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z3D64GPYF4G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z3DTV1VRJF00"}) SET p.content = "BREAKING: Craig Wright pushes Satoshi claim to new extremes, suing the judge for copyright infringement over the use of the word \"Bitcoin\" in court.", p.uri = "pubky:9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y/pubky.app/posts/2Z3DTV1VRJF00", p.createdAt = 1713300165190;
MATCH (u:User {id: "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y"}), (p:Post {id: "2Z3DTV1VRJF00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z3GMSDMQJK00"}) SET p.content = "̡͓̞ͅI̗̘̦͝n͇͇͙v̮̫ok̲̫̙͈i̖͙̭̹̠̞n̡̻̮̣̺g̲͈͙̭͙̬͎ ̰t͔̦h̞̲e̢̤ ͍̬̲͖f̴̘͕̣è͖ẹ̥̩l͖͔͚i͓͚̦͠n͖͍̗͓̳̮g͍ ̨o͚̪͡f̘̣̬ ̖̘͖̟͙̮c҉͔̫͖͓͇͖ͅh̵̤̣͚͔á̗̼͕ͅo̼̣̥s̱͈̺̖̦̻͢.̛̖̞̠̫̰", p.uri = "pubky:sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo/pubky.app/posts/2Z3GMSDMQJK00", p.createdAt = 1713349615177;
MATCH (u:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (p:Post {id: "2Z3GMSDMQJK00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z3HDTCCZB500"}) SET p.content = "hello", p.uri = "pubky:uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do/pubky.app/posts/2Z3HDTCCZB500", p.createdAt = 1713363375586;
MATCH (u:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (p:Post {id: "2Z3HDTCCZB500"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z3HPGPP4SW00"}) SET p.content = "Hello world! New official account!", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2Z3HPGPP4SW00", p.createdAt = 1713368157112;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z3HPGPP4SW00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z3JXJ4KW4700"}) SET p.content = "I have verified my public key using PGP.

-----BEGIN PGP SIGNED MESSAGE-----
Hash: SHA256
 
pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo
-----BEGIN PGP SIGNATURE-----

iQGzBAEBCAAdFiEERqOsg5X5Wm5tjx40gZ7e5Gc/PrsFAmYgPp4ACgkQgZ7e5Gc/
PrsOkwwAvkaVNpQ3ap2qJq5QChe8EAr+R8C0mTEH5G1zLkcrx7Jl0jKkoQ2UVX4Y
FBIjEqnFJm8+bLHiYTZQe7KnF/44rFxAxokhVHHlugdHmX5gN5eDGAQR5LZOmHZg
Cf2FTKrsk/B8iHD5C5peB87VFM3Gw6yt0UUi1daRKczYhX4BjQAn9dPTSn11/4GA
AClI/LxCMuSqeq2N9ylg8joq5d7ua0OSFklDWTIeTDHwF/UWwGbxWkbl5H7Lj0JG
xE3BwVEiDxNbK2wRNIgQd+a8bMvI+n/K9vIqUZlu9mb2Yo2SM18v4cra65Wfrujg
wQaCUSoJB8KHtS26EMfHKXPDsBthF4abMKPW/Ak+q8r6fx5gjYgqqaSbE07D1wus
n+TmcP+LdmcdZqpCBWtMLszXZ8b3C6zDF8ap3Y+xeKrsaI13Sk7hCtLLEPIlF3ut
WiJShi/Mta922owRth1+H0+YVrURZO5kD1d9hJPNZPabZgSvQVaZclIuLuaO8Cub
iI1FBRMC
=T5+w
-----END PGP SIGNATURE-----", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2Z3JXJ4KW4700", p.createdAt = 1713389622247;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z3JXJ4KW4700"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z3JXXZ2AEVG0"}) SET p.content = "Reminder: We need a better way to format messages. I'll open an issue to fix that!", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2Z3JXXZ2AEVG0", p.createdAt = 1713389825426;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z3JXXZ2AEVG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z3NMPKCJ07G0"}) SET p.content = "sovereign human action", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2Z3NMPKCJ07G0", p.createdAt = 1713437527652;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z3NMPKCJ07G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z3NTJPPA0D00"}) SET p.content = "Bad news, John: bots are already following and tagging each other.", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2Z3NTJPPA0D00", p.createdAt = 1713440759242;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z3NTJPPA0D00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z3Q6MJGQXT00"}) SET p.content = "Soooo much work!", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z3Q6MJGQXT00", p.createdAt = 1713464980617;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z3Q6MJGQXT00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z4F62WSMEE00"}) SET p.content = "Too many layout views. One to rule them all!", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2Z4F62WSMEE00", p.createdAt = 1713886889362;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z4F62WSMEE00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z4QSB3MG3300"}) SET p.content = "test", p.uri = "pubky:dujh8de33hbaqerg1ejyu9ynjh5yqfozm6iexsjdj3h9yfn3n3wy/pubky.app/posts/2Z4QSB3MG3300", p.createdAt = 1714038213321;
MATCH (u:User {id: "dujh8de33hbaqerg1ejyu9ynjh5yqfozm6iexsjdj3h9yfn3n3wy"}), (p:Post {id: "2Z4QSB3MG3300"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z4QT3VE88XG0"}) SET p.content = "test2", p.uri = "pubky:dujh8de33hbaqerg1ejyu9ynjh5yqfozm6iexsjdj3h9yfn3n3wy/pubky.app/posts/2Z4QT3VE88XG0", p.createdAt = 1714038638419;
MATCH (u:User {id: "dujh8de33hbaqerg1ejyu9ynjh5yqfozm6iexsjdj3h9yfn3n3wy"}), (p:Post {id: "2Z4QT3VE88XG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z4QT4DXBEJ00"}) SET p.content = "blablabla", p.uri = "pubky:dujh8de33hbaqerg1ejyu9ynjh5yqfozm6iexsjdj3h9yfn3n3wy/pubky.app/posts/2Z4QT4DXBEJ00", p.createdAt = 1714038648336;
MATCH (u:User {id: "dujh8de33hbaqerg1ejyu9ynjh5yqfozm6iexsjdj3h9yfn3n3wy"}), (p:Post {id: "2Z4QT4DXBEJ00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z5TYWGC9D700"}) SET p.content = "Fast fast", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z5TYWGC9D700", p.createdAt = 1714656987512;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z5TYWGC9D700"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z5W1DJT1MQG0"}) SET p.content = "Make Pubky public before it is ready", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z5W1DJT1MQG0", p.createdAt = 1714675972572;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z5W1DJT1MQG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z5W7KXCWQK00"}) SET p.content = "I deployed Pkarr server with rate limiting on all requests causing DHT queries, from either HTTP or UDP (resolvers).  Feels good to finish one stable robust layer, take it for granted and move to the next.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z5W7KXCWQK00", p.createdAt = 1714679379871;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z5W7KXCWQK00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z70P01HBYWG0"}) SET p.content = "Best programming language https://youtu.be/YYTB5_zBANg?si=H-1JawOdiIHZw-4w&t=326", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z70P01HBYWG0", p.createdAt = 1715320603531;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z70P01HBYWG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z744HF1S6D00"}) SET p.content = "Went to exchange some $ this morning... apparently, Türkiye is at the grams of gold stage of inflation because exchanges sell these now!", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z744HF1S6D00", p.createdAt = 1715381375983;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z744HF1S6D00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z75P8H2TBK00"}) SET p.content = "Denser UI >>>", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z75P8H2TBK00", p.createdAt = 1715408710246;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z75P8H2TBK00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z8M3ZVSVWD00"}) SET p.content = "New deployment May 20th ... Don't fly helicopters above mountains in bad weather.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z8M3ZVSVWD00", p.createdAt = 1716225498521;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z8M3ZVSVWD00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z8M80DMTV300"}) SET p.content = "Backing this one up...", p.uri = "pubky:hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o/pubky.app/posts/2Z8M80DMTV300", p.createdAt = 1716227707124;
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "2Z8M80DMTV300"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z8M96D3RAR00"}) SET p.content = "What do you call fake spaghetti? An impasta.", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2Z8M96D3RAR00", p.createdAt = 1716228359673;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z8M96D3RAR00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z8W2AFP242G0"}) SET p.content = "🍕", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z8W2AFP242G0", p.createdAt = 1716365318971;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z8W2AFP242G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z8ZCZGYSR2G0"}) SET p.content = "GM
", p.uri = "pubky:gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y/pubky.app/posts/2Z8ZCZGYSR2G0", p.createdAt = 1716423954548;
MATCH (u:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (p:Post {id: "2Z8ZCZGYSR2G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z8ZD1FXG1CG0"}) SET p.content = "Testing timestamp ", p.uri = "pubky:gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y/pubky.app/posts/2Z8ZD1FXG1CG0", p.createdAt = 1716423988349;
MATCH (u:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (p:Post {id: "2Z8ZD1FXG1CG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z91B580Q3300"}) SET p.content = "Test time", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z91B580Q3300", p.createdAt = 1716458137688;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z91B580Q3300"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z92MN2S4T9G0"}) SET p.content = "https://x.com/Rainmaker1973/status/1793665019869315499

when previews? :)", p.uri = "pubky:y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pubky.app/posts/2Z92MN2S4T9G0", p.createdAt = 1716480949742;
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z92MN2S4T9G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z96K6P2RASG0"}) SET p.content = "test tagging from posting", p.uri = "pubky:gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y/pubky.app/posts/2Z96K6P2RASG0", p.createdAt = 1716550521395;
MATCH (u:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (p:Post {id: "2Z96K6P2RASG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z96KAC1CX5G0"}) SET p.content = "#testtag test embedded tags", p.uri = "pubky:gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y/pubky.app/posts/2Z96KAC1CX5G0", p.createdAt = 1716550584723;
MATCH (u:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (p:Post {id: "2Z96KAC1CX5G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z97SZCC7PHG0"}) SET p.content = "I owe pk:y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy One US dollar.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z97SZCC7PHG0", p.createdAt = 1716571836403;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z97SZCC7PHG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z9D4393GKJG0"}) SET p.content = "https://miguelmedeiros.dev", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2Z9D4393GKJG0", p.createdAt = 1716665361853;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9D4393GKJG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z9D4C5BAJX00"}) SET p.content = "https://github.com/miguelmedeiros", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2Z9D4C5BAJX00", p.createdAt = 1716665514456;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9D4C5BAJX00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z9D6A0GDK900"}) SET p.content = "https://synonym.to", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2Z9D6A0GDK900", p.createdAt = 1716666577009;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9D6A0GDK900"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z9DG8TP8E100"}) SET p.content = "https://github.com/Nuhvi/pkarr", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2Z9DG8TP8E100", p.createdAt = 1716672054264;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9DG8TP8E100"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z9GBJ7WCFY00"}) SET p.content = "https://www.youtube.com/watch?v=Uc_HxKMKB_E", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2Z9GBJ7WCFY00", p.createdAt = 1716722243744;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9GBJ7WCFY00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z9GWEBYKY400"}) SET p.content = "https://x.com/halfin/status/1110302988", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2Z9GWEBYKY400", p.createdAt = 1716731523058;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9GWEBYKY400"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z9NE6HKC0T00"}) SET p.content = "Test https://app.pkarr.org", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z9NE6HKC0T00", p.createdAt = 1716811653001;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z9NE6HKC0T00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z9NE7JQSN900"}) SET p.content = "Test without protocol: app.pkarr.org", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z9NE7JQSN900", p.createdAt = 1716811670792;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z9NE7JQSN900"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z9NFB8Q1NC00"}) SET p.content = "Helllo my frends nbr 3", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/2Z9NFB8Q1NC00", p.createdAt = 1716812283886;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2Z9NFB8Q1NC00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z9NGRZJ9YC00"}) SET p.content = "#I%AIw5PbMCCdvXL", p.uri = "pubky:hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o/pubky.app/posts/2Z9NGRZJ9YC00", p.createdAt = 1716813069248;
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "2Z9NGRZJ9YC00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z9NKTBQF6B00"}) SET p.content = "Also posting my password now: 123456

Jay for president.", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/2Z9NKTBQF6B00", p.createdAt = 1716814742225;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2Z9NKTBQF6B00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z9NV94946Y00"}) SET p.content = "HIRING A MARKETING MANAGER



https://bitcoinerjobs.com/job/1498649-marketing-manager-synonym", p.uri = "pubky:ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo/pubky.app/posts/2Z9NV94946Y00", p.createdAt = 1716818844215;
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2Z9NV94946Y00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z9NX76YX3900"}) SET p.content = "This fuckin day!", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2Z9NX76YX3900", p.createdAt = 1716819910806;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z9NX76YX3900"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2Z9P8AN738C00"}) SET p.content = "wen wide view?", p.uri = "pubky:4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro/pubky.app/posts/2Z9P8AN738C00", p.createdAt = 1716826017313;
MATCH (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZA9KDTCB5Z00"}) SET p.content = "Still not a single controversial or unhinged take on this app.. tsk tsk tsk", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZA9KDTCB5Z00", p.createdAt = 1717166370473;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZA9KDTCB5Z00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAKTWDBB1300"}) SET p.content = "\"Everything is going to hell and nobody seems to care\" 

https://tonsky.me/blog/disenchantment/", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZAKTWDBB1300", p.createdAt = 1717346391327;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZAKTWDBB1300"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZASN5W6MZFG0"}) SET p.content = "⠀⠀⠀⠀⠀⠀⠀⠀⣀⣤⣴⣶⣾⣿⣿⣿⣿⣷⣶⣦⣤⣀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⣠⣴⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣦⣄⠀⠀⠀⠀⠀
⠀⠀⠀⣠⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⠿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣷⣄⠀⠀⠀
⠀⠀⣴⣿⣿⣿⣿⣿⣿⣿⠟⠿⠿⡿⠀⢰⣿⠁⢈⣿⣿⣿⣿⣿⣿⣿⣿⣦⠀⠀
⠀⣼⣿⣿⣿⣿⣿⣿⣿⣿⣤⣄⠀⠀⠀⠈⠉⠀⠸⠿⣿⣿⣿⣿⣿⣿⣿⣿⣧⠀
⢰⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡏⠀⠀⢠⣶⣶⣤⡀⠀⠈⢻⣿⣿⣿⣿⣿⣿⣿⡆
⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠃⠀⠀⠼⣿⣿⡿⠃⠀⠀⢸⣿⣿⣿⣿⣿⣿⣿⣷
⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡟⠀⠀⢀⣀⣀⠀⠀⠀⠀⢴⣿⣿⣿⣿⣿⣿⣿⣿⣿
⢿⣿⣿⣿⣿⣿⣿⣿⢿⣿⠁⠀⠀⣼⣿⣿⣿⣦⠀⠀⠈⢻⣿⣿⣿⣿⣿⣿⣿⡿

#bitcoin", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZASN5W6MZFG0", p.createdAt = 1717448808501;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZASN5W6MZFG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZATA2F2CVW00"}) SET p.content = "Emoji picker enabled! ✅
🧙‍♂️🐸🌽🍆🍕🍿🦀🍻🍷", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZATA2F2CVW00", p.createdAt = 1717460294783;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZATA2F2CVW00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAV28YDJSXG0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 1.15%
$69,187.19

🇧🇷 BTCBRL
🔼 0.54%
R$ 362.803,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAV28YDJSXG0", p.createdAt = 1717473600242;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAV28YDJSXG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAV8TGM8QB00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 18 sats/vB
🐢 +30 min : 18 sats/vB
🐌 +60 min : 17 sats/vB
🦥 +90 min : 14 sats/vB

🔥 Purge Limit : 7 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAV8TGM8QB00", p.createdAt = 1717477200611;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAV8TGM8QB00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAVDJTWS3P00"}) SET p.content = "Test if I can still post or if my account is broken again", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/2ZAVDJTWS3P00", p.createdAt = 1717479817463;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2ZAVDJTWS3P00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAVFC1DZHPG0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 0.33%
$68,914.29

🇧🇷 BTCBRL
🔽 -0.19%
R$ 361.501,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAVFC1DZHPG0", p.createdAt = 1717480800225;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAVFC1DZHPG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAVNXKHPBH00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 21 sats/vB
🐢 +30 min : 19 sats/vB
🐌 +60 min : 18 sats/vB
🦥 +90 min : 12 sats/vB

🔥 Purge Limit : 6 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAVNXKHPBH00", p.createdAt = 1717484400543;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAVNXKHPBH00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAVWF4CB5Q00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 0.21%
$69,013.62

🇧🇷 BTCBRL
🔽 -0.11%
R$ 362.097,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAVWF4CB5Q00", p.createdAt = 1717488000173;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAVWF4CB5Q00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAVYKGT508G0"}) SET p.content = "Testing previews

https://apple.com ", p.uri = "pubky:y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pubky.app/posts/2ZAVYKGT508G0", p.createdAt = 1717489175078;
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZAVYKGT508G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAW30PTDKBG0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 18 sats/vB
🐢 +30 min : 18 sats/vB
🐌 +60 min : 18 sats/vB
🦥 +90 min : 14 sats/vB

🔥 Purge Limit : 7 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAW30PTDKBG0", p.createdAt = 1717491600666;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAW30PTDKBG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAW9JBG9YA00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -0.43%
$68,717.92

🇧🇷 BTCBRL
🔽 -0.60%
R$ 360.652,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAW9JBG9YA00", p.createdAt = 1717495202363;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAW9JBG9YA00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAWG3STSFH00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 21 sats/vB
🐢 +30 min : 20 sats/vB
🐌 +60 min : 19 sats/vB
🦥 +90 min : 14 sats/vB

🔥 Purge Limit : 7 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAWG3STSFH00", p.createdAt = 1717498800648;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWG3STSFH00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAWPNE25GD00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -0.20%
$68,955.84

🇧🇷 BTCBRL
🔽 -0.28%
R$ 362.387,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAWPNE25GD00", p.createdAt = 1717502402102;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWPNE25GD00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAWR9RFTYPG0"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 3.09%

⏳ Countdown: 203,512 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 846,488

⏳ Days Until Halving: 1,413 days
🗓️ Halving Date: 17/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAWR9RFTYPG0", p.createdAt = 1717503301053;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWR9RFTYPG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAWV6RJGHB00"}) SET p.content = "When you run out of Bitcoin memes on camera

https://x.com/BeagleBitcoin/status/1797427730478461365", p.uri = "pubky:y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pubky.app/posts/2ZAWV6RJGHB00", p.createdAt = 1717504898825;
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZAWV6RJGHB00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAWVJGFGH400"}) SET p.content = "🦾 Bitcoin Difficulty Adjustment

🏁 Current Progress: 1786 of 2016 blocks

▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░ 88.59%

🗓️ Estimated Date: 5/6/2024

Current Change   : 🔼 1.15%
Previous Change : 🔼 1.48%

#Bitcoin #DifficultyAdjustment #GracePeriod", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAWVJGFGH400", p.createdAt = 1717505100639;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWVJGFGH400"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAWW07HF5MG0"}) SET p.content = "https://x.com/BeagleBitcoin/status/1797427730478461365", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZAWW07HF5MG0", p.createdAt = 1717505336358;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZAWW07HF5MG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAWW4G4VQSG0"}) SET p.content = "hmm previews are working... but if I add any text with the post it breaks. interesting, I will investigate.

https://x.com/BeagleBitcoin/status/1797427730478461365", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZAWW4G4VQSG0", p.createdAt = 1717505409698;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZAWW4G4VQSG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAWX6X2E1G00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 20 sats/vB
🐢 +30 min : 19 sats/vB
🐌 +60 min : 19 sats/vB
🦥 +90 min : 14 sats/vB

🔥 Purge Limit : 7 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAWX6X2E1G00", p.createdAt = 1717506000752;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWX6X2E1G00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAWY8KSKV600"}) SET p.content = "https://www.youtube.com/watch?v=HeehkH1TtZQ", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/2ZAWY8KSKV600", p.createdAt = 1717506579888;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2ZAWY8KSKV600"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAX0FNTWXA00"}) SET p.content = "⚡ Lightning Network

🪫 Total Capacity: 5,006 BTC
🪫 Avg. Capacity: 9,795,009 sats

🖥️ Total Nodes: 12,934
🤵‍♂️ Clearnet: 1,741
🕵️ Tor: 8,956
🔀 Channels: 51,107

💸 Avg. Fee: 762 ppm
💸 Avg. Base Fee: 949 msats

#Bitcoin #LightningNetwork", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAX0FNTWXA00", p.createdAt = 1717507800753;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAX0FNTWXA00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAX1DBDD5YG0"}) SET p.content = "https://en.wikipedia.org/wiki/Marvin_Heemeyer", p.uri = "pubky:gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y/pubky.app/posts/2ZAX1DBDD5YG0", p.createdAt = 1717508310555;
MATCH (u:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (p:Post {id: "2ZAX1DBDD5YG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAX3RG4X0D00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -0.29%
$69,620.01

🇧🇷 BTCBRL
🔽 -0.26%
R$ 366.793,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAX3RG4X0D00", p.createdAt = 1717509601586;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAX3RG4X0D00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAXBYD491Q00"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 3.10%

⏳ Countdown: 203,492 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 846,508

⏳ Days Until Halving: 1,413 days
🗓️ Halving Date: 17/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAXBYD491Q00", p.createdAt = 1717514101091;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXBYD491Q00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAXGVGPD97G0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 1.20%
$70,443.82

🇧🇷 BTCBRL
🔼 1.16%
R$ 370.079,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAXGVGPD97G0", p.createdAt = 1717516800245;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXGVGPD97G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAXJWQJ71Z00"}) SET p.content = "My birthday isn't soon but still, I want one of these https://newsletter.pragmaticengineer.com/p/oxide", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZAXJWQJ71Z00", p.createdAt = 1717517920625;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZAXJWQJ71Z00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAXQD2WYZJG0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 31 sats/vB
🐢 +30 min : 28 sats/vB
🐌 +60 min : 25 sats/vB
🦥 +90 min : 16 sats/vB

🔥 Purge Limit : 8 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAXQD2WYZJG0", p.createdAt = 1717520400612;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXQD2WYZJG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAXRP9Y7M500"}) SET p.content = "I'm thinking about buying one of these standing desks:

https://www.geniodesks.com.br/produto-mesa-com-regulagem-de-altura-geniodesk-pro", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZAXRP9Y7M500", p.createdAt = 1717521108766;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZAXRP9Y7M500"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAXS1G7H4WG0"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 3.10%

⏳ Countdown: 203,485 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 846,515

⏳ Days Until Halving: 1,413 days
🗓️ Halving Date: 17/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAXS1G7H4WG0", p.createdAt = 1717521301121;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXS1G7H4WG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAXXYM0Z7MG0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 1.90%
$70,564.57

🇧🇷 BTCBRL
🔼 2.50%
R$ 372.210,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAXXYM0Z7MG0", p.createdAt = 1717524000398;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXXYM0Z7MG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAY4G649JZ00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 22 sats/vB
🐢 +30 min : 22 sats/vB
🐌 +60 min : 21 sats/vB
🦥 +90 min : 16 sats/vB

🔥 Purge Limit : 8 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAY4G649JZ00", p.createdAt = 1717527600710;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAY4G649JZ00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAYB1Q0G22G0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 2.01%
$70,481.43

🇧🇷 BTCBRL
🔼 2.64%
R$ 372.893,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAYB1Q0G22G0", p.createdAt = 1717531200366;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYB1Q0G22G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAYHK9CJ4AG0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 45 sats/vB
🐢 +30 min : 42 sats/vB
🐌 +60 min : 35 sats/vB
🦥 +90 min : 16 sats/vB

🔥 Purge Limit : 8 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAYHK9CJ4AG0", p.createdAt = 1717534800825;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYHK9CJ4AG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAYPGE6MHYG0"}) SET p.content = "🦾 Bitcoin Difficulty Adjustment

🏁 Current Progress: 1841 of 2016 blocks

▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░ 91.32%

🗓️ Estimated Date: 5/6/2024

Current Change   : 🔼 1.17%
Previous Change : 🔼 1.48%

#Bitcoin #DifficultyAdjustment #GracePeriod", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAYPGE6MHYG0", p.createdAt = 1717537500649;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYPGE6MHYG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAYR4T072DG0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 2.25%
$70,601.10

🇧🇷 BTCBRL
🔼 2.95%
R$ 373.800,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAYR4T072DG0", p.createdAt = 1717538400337;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYR4T072DG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAYSS7SPMDG0"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 3.12%

⏳ Countdown: 203,455 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 846,545

⏳ Days Until Halving: 1,413 days
🗓️ Halving Date: 17/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAYSS7SPMDG0", p.createdAt = 1717539301097;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYSS7SPMDG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAYVDM48TCG0"}) SET p.content = "⚡ Lightning Network

🪫 Total Capacity: 5,001 BTC
🪫 Avg. Capacity: 9,789,528 sats

🖥️ Total Nodes: 12,918
🤵‍♂️ Clearnet: 1,742
🕵️ Tor: 8,945
🔀 Channels: 51,085

💸 Avg. Fee: 763 ppm
💸 Avg. Base Fee: 950 msats

#Bitcoin #LightningNetwork", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAYVDM48TCG0", p.createdAt = 1717540201070;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYVDM48TCG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAYYPCEEWWG0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 25 sats/vB
🐢 +30 min : 23 sats/vB
🐌 +60 min : 22 sats/vB
🦥 +90 min : 16 sats/vB

🔥 Purge Limit : 8 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAYYPCEEWWG0", p.createdAt = 1717542000833;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYYPCEEWWG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAZ57X95J500"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 2.51%
$70,548.00

🇧🇷 BTCBRL
🔼 3.22%
R$ 372.869,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAZ57X95J500", p.createdAt = 1717545600463;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZ57X95J500"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAZ6WB0WNEG0"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 3.12%

⏳ Countdown: 203,445 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 846,555

⏳ Days Until Halving: 1,413 days
🗓️ Halving Date: 17/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAZ6WB0WNEG0", p.createdAt = 1717546501194;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZ6WB0WNEG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAZBSFANCN00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 20 sats/vB
🐢 +30 min : 20 sats/vB
🐌 +60 min : 20 sats/vB
🦥 +90 min : 14 sats/vB

🔥 Purge Limit : 7 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAZBSFANCN00", p.createdAt = 1717549200745;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZBSFANCN00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAZJB037XFG0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 2.63%
$70,994.17

🇧🇷 BTCBRL
🔼 3.03%
R$ 373.983,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAZJB037XFG0", p.createdAt = 1717552800340;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZJB037XFG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAZRWNFQGDG0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 22 sats/vB
🐢 +30 min : 21 sats/vB
🐌 +60 min : 21 sats/vB
🦥 +90 min : 12 sats/vB

🔥 Purge Limit : 6 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAZRWNFQGDG0", p.createdAt = 1717556402416;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZRWNFQGDG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZAZZE6EDRQ00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 2.56%
$70,960.00

🇧🇷 BTCBRL
🔼 3.08%
R$ 373.961,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZAZZE6EDRQ00", p.createdAt = 1717560002114;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZZE6EDRQ00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB05TSWXABG0"}) SET p.content = "When notifications?", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/2ZB05TSWXABG0", p.createdAt = 1717563517251;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2ZB05TSWXABG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB05ZR9VTK00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 24 sats/vB
🐢 +30 min : 23 sats/vB
🐌 +60 min : 22 sats/vB
🦥 +90 min : 12 sats/vB

🔥 Purge Limit : 6 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZB05ZR9VTK00", p.createdAt = 1717563602293;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB05ZR9VTK00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB0CHACWC6G0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 3.03%
$70,999.99

🇧🇷 BTCBRL
🔼 3.42%
R$ 373.862,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZB0CHACWC6G0", p.createdAt = 1717567202600;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB0CHACWC6G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB0K2V4Y6KG0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 23 sats/vB
🐢 +30 min : 23 sats/vB
🐌 +60 min : 22 sats/vB
🦥 +90 min : 16 sats/vB

🔥 Purge Limit : 8 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZB0K2V4Y6KG0", p.createdAt = 1717570802187;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB0K2V4Y6KG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB0PB2KVV700"}) SET p.content = "Looking for a new destination for nomads https://en.wikipedia.org/wiki/List_of_potentially_habitable_exoplanets", p.uri = "pubky:gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y/pubky.app/posts/2ZB0PB2KVV700", p.createdAt = 1717572592902;
MATCH (u:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (p:Post {id: "2ZB0PB2KVV700"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB0SMCYX7J00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 3.15%
$71,190.01

🇧🇷 BTCBRL
🔼 3.52%
R$ 374.858,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZB0SMCYX7J00", p.createdAt = 1717574402342;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB0SMCYX7J00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB0T9K76NYG0"}) SET p.content = "In the children's game, paper beats rock.
But in reality, rock beats paper.", p.uri = "pubky:9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y/pubky.app/posts/2ZB0T9K76NYG0", p.createdAt = 1717574766479;
MATCH (u:User {id: "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y"}), (p:Post {id: "2ZB0T9K76NYG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB105Y98Q0G0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 30 sats/vB
🐢 +30 min : 28 sats/vB
🐌 +60 min : 27 sats/vB
🦥 +90 min : 18 sats/vB

🔥 Purge Limit : 9 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZB105Y98Q0G0", p.createdAt = 1717578002235;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB105Y98Q0G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB16QG14K1G0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 3.18%
$70,880.49

🇧🇷 BTCBRL
🔼 3.83%
R$ 374.549,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZB16QG14K1G0", p.createdAt = 1717581602355;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB16QG14K1G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB1D91D0AAG0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 34 sats/vB
🐢 +30 min : 32 sats/vB
🐌 +60 min : 31 sats/vB
🦥 +90 min : 18 sats/vB

🔥 Purge Limit : 9 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZB1D91D0AAG0", p.createdAt = 1717585202274;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1D91D0AAG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB1KTN60RN00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 2.91%
$70,971.71

🇧🇷 BTCBRL
🔼 3.65%
R$ 375.600,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZB1KTN60RN00", p.createdAt = 1717588803486;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1KTN60RN00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB1NF3BF7GG0"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 3.15%

⏳ Countdown: 203,386 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 846,614

⏳ Days Until Halving: 1,412 days
🗓️ Halving Date: 17/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZB1NF3BF7GG0", p.createdAt = 1717589704447;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1NF3BF7GG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB1RQTAXFR00"}) SET p.content = "🦾 Bitcoin Difficulty Adjustment

🏁 Current Progress: 1915 of 2016 blocks

▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░ 94.99%

🗓️ Estimated Date: 6/6/2024

Current Change   : 🔼 0.28%
Previous Change : 🔼 1.48%

#Bitcoin #DifficultyAdjustment #GracePeriod", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZB1RQTAXFR00", p.createdAt = 1717591503492;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1RQTAXFR00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB1S9AVTTPG0"}) SET p.content = "Telegram is spyware. What's the alternative?", p.uri = "pubky:9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y/pubky.app/posts/2ZB1S9AVTTPG0", p.createdAt = 1717591804424;
MATCH (u:User {id: "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y"}), (p:Post {id: "2ZB1S9AVTTPG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB1TC6J2KK00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 39 sats/vB
🐢 +30 min : 35 sats/vB
🐌 +60 min : 32 sats/vB
🦥 +90 min : 18 sats/vB

🔥 Purge Limit : 9 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZB1TC6J2KK00", p.createdAt = 1717592403408;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1TC6J2KK00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB1W7ZPCGHG0"}) SET p.content = "I think bringing Murray Rothbot to Pubky was great!

We've already caught some bugs thanks to it. I noticed a side effect: bc it creates multiple posts, I believe it has made people less shy about posting and encouraged them to use Pubky more!

Or maybe it's just my imagination! 🤣", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZB1W7ZPCGHG0", p.createdAt = 1717593430515;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZB1W7ZPCGHG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB1XN0942200"}) SET p.content = "⚡ Lightning Network

🪫 Total Capacity: 5,015 BTC
🪫 Avg. Capacity: 9,813,158 sats

🖥️ Total Nodes: 12,920
🤵‍♂️ Clearnet: 1,744
🕵️ Tor: 8,941
🔀 Channels: 51,104

💸 Avg. Fee: 762 ppm
💸 Avg. Base Fee: 948 msats

#Bitcoin #LightningNetwork", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZB1XN0942200", p.createdAt = 1717594203923;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1XN0942200"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB20XR1N4D00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 1.52%
$70,721.06

🇧🇷 BTCBRL
🔼 2.01%
R$ 374.390,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZB20XR1N4D00", p.createdAt = 1717596003389;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB20XR1N4D00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB26RT500X00"}) SET p.content = "gg", p.uri = "pubky:ym1rn4rfjg8857y13uuehc4tw6sfqtzpu881ycdj7iiyo6kqsspy/pubky.app/posts/2ZB26RT500X00", p.createdAt = 1717599217154;
MATCH (u:User {id: "ym1rn4rfjg8857y13uuehc4tw6sfqtzpu881ycdj7iiyo6kqsspy"}), (p:Post {id: "2ZB26RT500X00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZB9WRZ6B6200"}) SET p.content = "Sev is going to love this! https://daylightcomputer.com/", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZB9WRZ6B6200", p.createdAt = 1717734459792;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZB9WRZ6B6200"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZBAKNPJ2D900"}) SET p.content = "This is so outrageous imagine all the suffering caused by this? https://www.science.org/content/article/researchers-plan-retract-landmark-alzheimers-paper-containing-doctored-images", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZBAKNPJ2D900", p.createdAt = 1717747048001;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZBAKNPJ2D900"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZBC5NVR83MG0"}) SET p.content = "I must evolve a bit more
Think of love not war
Think of peers not fears
Think of trust in the source
Not the laws that enforce", p.uri = "pubky:9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y/pubky.app/posts/2ZBC5NVR83MG0", p.createdAt = 1717774538579;
MATCH (u:User {id: "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y"}), (p:Post {id: "2ZBC5NVR83MG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZBCBSQPKGGG0"}) SET p.content = "I don't know how did I convince the Youtube algorithm to recommend this to me, but ... I am kinda proud of myself https://www.youtube.com/watch?v=6Air1H61eUI", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZBCBSQPKGGG0", p.createdAt = 1717777903659;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZBCBSQPKGGG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZBFDYTFD8C00"}) SET p.content = "A capitalist society complaining about \"artificially cheap exports\" from another country, is a society that is admitting it doesn't know what to do with wealth, a stagnate society retreating to zero-sum games.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZBFDYTFD8C00", p.createdAt = 1717831867118;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZBFDYTFD8C00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZBFGACDZQR00"}) SET p.content = "Read on Twitter: \"education can be fun or effective but not both.\"", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZBFGACDZQR00", p.createdAt = 1717833165248;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZBFGACDZQR00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZBFHEWJQWT00"}) SET p.content = "Injection molding is SIMD for atoms.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZBFHEWJQWT00", p.createdAt = 1717833792393;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZBFHEWJQWT00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZBFXCKJ48RG0"}) SET p.content = "How long can we really go... lalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalallalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalallalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalallalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalallalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalallalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalallalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalallalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalallalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalalal", p.uri = "pubky:9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y/pubky.app/posts/2ZBFXCKJ48RG0", p.createdAt = 1717840350260;
MATCH (u:User {id: "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y"}), (p:Post {id: "2ZBFXCKJ48RG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZBKXMD487E00"}) SET p.content = "Any sufficiently advanced digital identity is indistinguishable from domain names.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZBKXMD487E00", p.createdAt = 1717910852990;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZBKXMD487E00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZBPTGCYY2600"}) SET p.content = "https://youtu.be/ig2HoJ7lenM?si=sYFbksrfk6qQxnMb", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZBPTGCYY2600", p.createdAt = 1717961911472;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZBPTGCYY2600"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZC3BRC2WZZ00"}) SET p.content = "Test https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:5dn6hroc3v7i53cz6hpq3zgv/bafkreiauf4j6gp4tm4rrmjgdopqelzni42vh3zrwlsqm2tkgaoiv3olkui@jpeg ", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZC3BRC2WZZ00", p.createdAt = 1718182500521;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZC3BRC2WZZ00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZC76V8WZZ6G0"}) SET p.content = "Testing Replies and Reposts!", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZC76V8WZZ6G0", p.createdAt = 1718250170317;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZC76V8WZZ6G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZC76VY5G5K00"}) SET p.content = "First reply ever!", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZC76VY5G5K00", p.createdAt = 1718250181734;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZC76VY5G5K00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZC76V8WZZ6G0"}), (p2:Post {id: "2ZC76VY5G5K00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZC76WPRB2P00"}) SET p.content = "First reply of a reply ever!", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZC76WPRB2P00", p.createdAt = 1718250194935;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZC76WPRB2P00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZC76VY5G5K00"}), (p2:Post {id: "2ZC76WPRB2P00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZC76ZYFBWAG0"}) SET p.content = "", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZC76ZYFBWAG0", p.createdAt = 1718250250619;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZC76ZYFBWAG0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZC76V8WZZ6G0"}), (p2:Post {id: "2ZC76ZYFBWAG0"}) MERGE (p2)-[:REPOST_OF]->(p1);
MERGE (p:Post {id: "2ZC77EG4BA900"}) SET p.content = "Test quote repost!", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZC77EG4BA900", p.createdAt = 1718250500616;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZC77EG4BA900"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZB20XR1N4D00"}), (p2:Post {id: "2ZC77EG4BA900"}) MERGE (p2)-[:REPOST_OF]->(p1);
MERGE (p:Post {id: "2ZC8704TDY900"}) SET p.content = "👀", p.uri = "pubky:sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo/pubky.app/posts/2ZC8704TDY900", p.createdAt = 1718267846211;
MATCH (u:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (p:Post {id: "2ZC8704TDY900"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZBFXCKJ48RG0"}), (p2:Post {id: "2ZC8704TDY900"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZC872SDGC600"}) SET p.content = "Good luck to those in Prague.
I shall be watching from the sidelines.
✌️", p.uri = "pubky:sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo/pubky.app/posts/2ZC872SDGC600", p.createdAt = 1718267891629;
MATCH (u:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (p:Post {id: "2ZC872SDGC600"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZC8T831Z2M00"}) SET p.content = "Can't even tell who is who on this app any more.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZC8T831Z2M00", p.createdAt = 1718278428064;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZC8T831Z2M00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZC872SDGC600"}), (p2:Post {id: "2ZC8T831Z2M00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZC92PTXD0EG0"}) SET p.content = "Does nesting work now?", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZC92PTXD0EG0", p.createdAt = 1718283079437;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZC92PTXD0EG0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZC8T831Z2M00"}), (p2:Post {id: "2ZC92PTXD0EG0"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZC999PTF4N00"}) SET p.content = "No", p.uri = "pubky:hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o/pubky.app/posts/2ZC999PTF4N00", p.createdAt = 1718286702192;
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "2ZC999PTF4N00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZC92PTXD0EG0"}), (p2:Post {id: "2ZC999PTF4N00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZC99EGRJ8CG0"}) SET p.content = "undefined", p.uri = "pubky:hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o/pubky.app/posts/2ZC99EGRJ8CG0", p.createdAt = 1718286784838;
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "2ZC99EGRJ8CG0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZC77EG4BA900"}), (p2:Post {id: "2ZC99EGRJ8CG0"}) MERGE (p2)-[:REPOST_OF]->(p1);
MERGE (p:Post {id: "2ZC99KR7PMQG0"}) SET p.content = "Replying to post makes you look crazy on the home page", p.uri = "pubky:hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o/pubky.app/posts/2ZC99KR7PMQG0", p.createdAt = 1718286874750;
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "2ZC99KR7PMQG0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZC999PTF4N00"}), (p2:Post {id: "2ZC99KR7PMQG0"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZC99Q5B4TS00"}) SET p.content = "Does it?", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZC99Q5B4TS00", p.createdAt = 1718286933327;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZC99Q5B4TS00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZC99KR7PMQG0"}), (p2:Post {id: "2ZC99Q5B4TS00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZCA9DZ6SZMG0"}) SET p.content = "https://stacker.news/items/572787", p.uri = "pubky:sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo/pubky.app/posts/2ZCA9DZ6SZMG0", p.createdAt = 1718304367600;
MATCH (u:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (p:Post {id: "2ZCA9DZ6SZMG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZCEHJ3M7DRG0"}) SET p.content = "thread root", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZCEHJ3M7DRG0", p.createdAt = 1718379205483;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZCEHJ3M7DRG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZCEHJM8SJW00"}) SET p.content = "thread reply", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZCEHJM8SJW00", p.createdAt = 1718379214417;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZCEHJM8SJW00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZCEHJ3M7DRG0"}), (p2:Post {id: "2ZCEHJM8SJW00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZCEHK3P6KX00"}) SET p.content = "reply of a reply", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZCEHK3P6KX00", p.createdAt = 1718379222696;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZCEHK3P6KX00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZCEHJM8SJW00"}), (p2:Post {id: "2ZCEHK3P6KX00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZCVS8MYJXH00"}) SET p.content = "Wuhuu", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/2ZCVS8MYJXH00", p.createdAt = 1718612139449;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2ZCVS8MYJXH00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZCEHK3P6KX00"}), (p2:Post {id: "2ZCVS8MYJXH00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZCVXBPATDZG0"}) SET p.content = "undefined", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/2ZCVXBPATDZG0", p.createdAt = 1718614390755;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2ZCVXBPATDZG0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZCEHJM8SJW00"}), (p2:Post {id: "2ZCVXBPATDZG0"}) MERGE (p2)-[:REPOST_OF]->(p1);
MERGE (p:Post {id: "2ZCW1TGR5BKG0"}) SET p.content = "I am told we can reply now!", p.uri = "pubky:y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pubky.app/posts/2ZCW1TGR5BKG0", p.createdAt = 1718616844478;
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZCW1TGR5BKG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZCWD3684B700"}) SET p.content = "Yes we can", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZCWD3684B700", p.createdAt = 1718623040774;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZCWD3684B700"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZCW1TGR5BKG0"}), (p2:Post {id: "2ZCWD3684B700"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZCWWEQ4TB600"}) SET p.content = "I just realized, the first attempt at Pkarr, was 2 years ago https://github.com/Nuhvi/slashtags-seeder-records", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZCWWEQ4TB600", p.createdAt = 1718631485161;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZCWWEQ4TB600"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZCWXSXM1FHG0"}) SET p.content = "\"We've come a long way from where we began\"
https://www.youtube.com/watch?v=NDEWXnMRq3c", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/2ZCWXSXM1FHG0", p.createdAt = 1718632227372;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2ZCWXSXM1FHG0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZCWWEQ4TB600"}), (p2:Post {id: "2ZCWXSXM1FHG0"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZCWZ5545FA00"}) SET p.content = "😅💯", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZCWZ5545FA00", p.createdAt = 1718632970135;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZCWZ5545FA00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZCWXSXM1FHG0"}), (p2:Post {id: "2ZCWZ5545FA00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZD0DPCJSXH00"}) SET p.content = "We might be free from MacBooks soon https://www.youtube.com/watch?v=rSx0WZfDbE0 (sorry native devs you are stuck I guess).", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZD0DPCJSXH00", p.createdAt = 1718693739336;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD0DPCJSXH00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZD19WPE9GC00"}) SET p.content = "You should stay logged in even if we restarted the server!", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZD19WPE9GC00", p.createdAt = 1718709240871;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD19WPE9GC00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZD1BGEH4M000"}) SET p.content = "Never", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZD1BGEH4M000", p.createdAt = 1718710129977;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD1BGEH4M000"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD1BMZB80PG0"}), (p2:Post {id: "2ZD1BGEH4M000"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZD1BMZB80PG0"}) SET p.content = "https://x.com/_miguelmedeiros/status/1803027346733105273", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZD1BMZB80PG0", p.createdAt = 1718710207724;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD1BMZB80PG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZD1N7WRVKG00"}) SET p.content = "undefined", p.uri = "pubky:h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy/pubky.app/posts/2ZD1N7WRVKG00", p.createdAt = 1718715480561;
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZD1N7WRVKG00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD1BMZB80PG0"}), (p2:Post {id: "2ZD1N7WRVKG00"}) MERGE (p2)-[:REPOST_OF]->(p1);
MERGE (p:Post {id: "2ZD2GBAGJ4XG0"}) SET p.content = "https://x.com/RadarHits/status/1803034836388528448", p.uri = "pubky:y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pubky.app/posts/2ZD2GBAGJ4XG0", p.createdAt = 1718730382885;
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZD2GBAGJ4XG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZD2HGQ86Z0G0"}) SET p.content = "A whole blog documenting every rug pull letter companies issue when they get acquired and start shutting their service https://ourincrediblejourney.tumblr.com/", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZD2HGQ86Z0G0", p.createdAt = 1718731025380;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD2HGQ86Z0G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZD2YWZJ0RMG0"}) SET p.content = "Should I bring Murray Rothbot to Pubky again?", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZD2YWZJ0RMG0", p.createdAt = 1718738382823;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD2YWZJ0RMG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZD52PVKVSY00"}) SET p.content = "I love that I can actually see and play embedded videos. X is always throttling YT posts.", p.uri = "pubky:y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pubky.app/posts/2ZD52PVKVSY00", p.createdAt = 1718775661023;
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZD52PVKVSY00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZCWXSXM1FHG0"}), (p2:Post {id: "2ZD52PVKVSY00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZD54TZR9XS00"}) SET p.content = "This could be us someday, fud billboards against Pubky and the dangers of open web.

https://x.com/EleanorTerrett/status/1803145163705081965", p.uri = "pubky:y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pubky.app/posts/2ZD54TZR9XS00", p.createdAt = 1718776831476;
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZD54TZR9XS00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZD57HSGPJ300"}) SET p.content = "Rethinking the value of the \"team\" page.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZD57HSGPJ300", p.createdAt = 1718778322776;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD57HSGPJ300"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD54TZR9XS00"}), (p2:Post {id: "2ZD57HSGPJ300"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZD58JSJP3K00"}) SET p.content = "We need a better feed algorithm first, otherwise this gets too spamy sir", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/2ZD58JSJP3K00", p.createdAt = 1718778889745;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2ZD58JSJP3K00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD2YWZJ0RMG0"}), (p2:Post {id: "2ZD58JSJP3K00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZD58WJ3EK900"}) SET p.content = "I love it too. I wonder how big the user drop off from pubky is tho.", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/2ZD58WJ3EK900", p.createdAt = 1718779057529;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2ZD58WJ3EK900"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD52PVKVSY00"}), (p2:Post {id: "2ZD58WJ3EK900"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZD59C8JCZ100"}) SET p.content = "How is Linux support for ARM?", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/2ZD59C8JCZ100", p.createdAt = 1718779327289;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2ZD59C8JCZ100"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD0DPCJSXH00"}), (p2:Post {id: "2ZD59C8JCZ100"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZD5HMJTSJCG0"}) SET p.content = "No idea!", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZD5HMJTSJCG0", p.createdAt = 1718783868285;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD5HMJTSJCG0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD59C8JCZ100"}), (p2:Post {id: "2ZD5HMJTSJCG0"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZD5KPB39J2G0"}) SET p.content = "
EU interior ministers want #chatcontrol to scan us, but are seeking to exempt themselves because of the dangers 

https://www.eureporter.co/business/data/mass-surveillance-data/2024/04/15/leak-eu-interior-ministers-want-to-exempt-themselves-from-chat-control-bulk-scanning-of-private-messages/

🤡", p.uri = "pubky:ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo/pubky.app/posts/2ZD5KPB39J2G0", p.createdAt = 1718784998004;
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2ZD5KPB39J2G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZD5RF0PVBD00"}) SET p.content = "\"It’s been our priority not only to support Linux on our premium-tier SoCs, but to support it pronto.\"

https://www.qualcomm.com/developer/blog/2024/05/upstreaming-linux-kernel-support-for-the-snapdragon-x-elite", p.uri = "pubky:sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo/pubky.app/posts/2ZD5RF0PVBD00", p.createdAt = 1718787620946;
MATCH (u:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (p:Post {id: "2ZD5RF0PVBD00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD59C8JCZ100"}), (p2:Post {id: "2ZD5RF0PVBD00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZD5RQT277A00"}) SET p.content = "Bitcoin is for enemies

https://www.youtube.com/watch?v=JiR7924Kuiw", p.uri = "pubky:ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo/pubky.app/posts/2ZD5RQT277A00", p.createdAt = 1718787771998;
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2ZD5RQT277A00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZD5SH607R0G0"}) SET p.content = "Has anyone tried umbrelOS on x86 yet?", p.uri = "pubky:sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo/pubky.app/posts/2ZD5SH607R0G0", p.createdAt = 1718788207903;
MATCH (u:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (p:Post {id: "2ZD5SH607R0G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZD5TBQZFC900"}) SET p.content = "Encryption is binary.", p.uri = "pubky:sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo/pubky.app/posts/2ZD5TBQZFC900", p.createdAt = 1718788664231;
MATCH (u:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (p:Post {id: "2ZD5TBQZFC900"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD5KPB39J2G0"}), (p2:Post {id: "2ZD5TBQZFC900"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZD61CM72ZH00"}) SET p.content = "https://x.com/KarinaVinnikova/status/1802980985056710732", p.uri = "pubky:sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo/pubky.app/posts/2ZD61CM72ZH00", p.createdAt = 1718792527682;
MATCH (u:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (p:Post {id: "2ZD61CM72ZH00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZD65VYQMTY00"}) SET p.content = "I hate this so much... maybe we should just go offline forever.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZD65VYQMTY00", p.createdAt = 1718794990049;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD65VYQMTY00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD61CM72ZH00"}), (p2:Post {id: "2ZD65VYQMTY00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZD65XQCEY600"}) SET p.content = "undefined", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZD65XQCEY600", p.createdAt = 1718795020464;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD65XQCEY600"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD5KPB39J2G0"}), (p2:Post {id: "2ZD65XQCEY600"}) MERGE (p2)-[:REPOST_OF]->(p1);
MERGE (p:Post {id: "2ZD65ZEQQKXG0"}) SET p.content = "undefined", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZD65ZEQQKXG0", p.createdAt = 1718795050181;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD65ZEQQKXG0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD5RF0PVBD00"}), (p2:Post {id: "2ZD65ZEQQKXG0"}) MERGE (p2)-[:REPOST_OF]->(p1);
MERGE (p:Post {id: "2ZD67W27BHB00"}) SET p.content = "We already have the \"following\" filter, so this spam from Murray could be interesting to encourage people to start following each other and filter by followers, right?", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZD67W27BHB00", p.createdAt = 1718796091435;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD67W27BHB00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD58JSJP3K00"}), (p2:Post {id: "2ZD67W27BHB00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZD6BK61CRT00"}) SET p.content = "It's time to 🍄!", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZD6BK61CRT00", p.createdAt = 1718798137887;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD6BK61CRT00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZD6CPRQTAC00"}) SET p.content = "Fake news", p.uri = "pubky:ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo/pubky.app/posts/2ZD6CPRQTAC00", p.createdAt = 1718798749223;
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2ZD6CPRQTAC00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZCW1TGR5BKG0"}), (p2:Post {id: "2ZD6CPRQTAC00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZD6CZS8Z0SG0"}) SET p.content = "and repost 🚀", p.uri = "pubky:h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy/pubky.app/posts/2ZD6CZS8Z0SG0", p.createdAt = 1718798904129;
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZD6CZS8Z0SG0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZCW1TGR5BKG0"}), (p2:Post {id: "2ZD6CZS8Z0SG0"}) MERGE (p2)-[:REPOST_OF]->(p1);
MERGE (p:Post {id: "2ZD6EH3PEF000"}) SET p.content = "The  EU recommends opening an excessive deficit procedure against France

https://www.bfmtv.com/economie/economie-social/union-europeenne/l-ue-recommande-d-ouvrir-une-procedure-pour-deficit-public-excessif-contre-la-france_AD-202406190386.html?at_brand=BFMTV&at_compte=BFMTV&at_plateforme=twitter", p.uri = "pubky:ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo/pubky.app/posts/2ZD6EH3PEF000", p.createdAt = 1718799751537;
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2ZD6EH3PEF000"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZD6JHJQ6MZG0"}) SET p.content = "undefined", p.uri = "pubky:h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy/pubky.app/posts/2ZD6JHJQ6MZG0", p.createdAt = 1718801958627;
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZD6JHJQ6MZG0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD6BK61CRT00"}), (p2:Post {id: "2ZD6JHJQ6MZG0"}) MERGE (p2)-[:REPOST_OF]->(p1);
MERGE (p:Post {id: "2ZD6M4WB0X6G0"}) SET p.content = "Could this replace Asana?

https://slack.com/intl/en-gb/blog/news/introducing-slack-lists", p.uri = "pubky:sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo/pubky.app/posts/2ZD6M4WB0X6G0", p.createdAt = 1718802839964;
MATCH (u:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (p:Post {id: "2ZD6M4WB0X6G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZD9VJK97YN00"}) SET p.content = "We should demand sovereignty over our computers.

https://www.youtube.com/watch?v=c52pKpYeZ74", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZD9VJK97YN00", p.createdAt = 1718859700470;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD9VJK97YN00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDDMJ18KB700"}) SET p.content = "If you would like to know what's going on in France atm

https://x.com/ojblanchard1/status/1804052254879572054", p.uri = "pubky:ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo/pubky.app/posts/2ZDDMJ18KB700", p.createdAt = 1718926211249;
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2ZDDMJ18KB700"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDDX8J4JAWG0"}) SET p.content = "yes.", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/2ZDDX8J4JAWG0", p.createdAt = 1718930996311;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "2ZDDX8J4JAWG0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD6M4WB0X6G0"}), (p2:Post {id: "2ZDDX8J4JAWG0"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZDDXBWR520G0"}) SET p.content = "🍄 = level up, Mario developers knew it all along.", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/2ZDDXBWR520G0", p.createdAt = 1718931053548;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "2ZDDXBWR520G0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD6BK61CRT00"}), (p2:Post {id: "2ZDDXBWR520G0"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZDDYCKG90H00"}) SET p.content = "Eveyone Pubkys", p.uri = "pubky:hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty/pubky.app/posts/2ZDDYCKG90H00", p.createdAt = 1718931615520;
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "2ZDDYCKG90H00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDH5QAWVHT00"}) SET p.content = "Pool: How do you most frequently create posts on Pubky?

🅰️ - Using the form at the top of the timeline
🅱️ - Using the button at the bottom right", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZDH5QAWVHT00", p.createdAt = 1718988424727;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZDH5QAWVHT00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDMBYCN25X00"}) SET p.content = "Nice, using tags for a poll :)", p.uri = "pubky:y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pubky.app/posts/2ZDMBYCN25X00", p.createdAt = 1719044621022;
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZDMBYCN25X00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZDH5QAWVHT00"}), (p2:Post {id: "2ZDMBYCN25X00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZDMP31SBG500"}) SET p.content = "I wish we started by building a real-time RSS reader for Pubky.. that would have been an easy win. The thing about feeds is that they are extremely cheap, they neither need hydration (adding likes, tags, etc.) nor filtering (per user pov). You consume them as is, making Indexers unnecessary.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZDMP31SBG500", p.createdAt = 1719050198646;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDMP31SBG500"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDMPCWN784G0"}) SET p.content = "Another way to express this post is: I wish there was more exciting applications where low-latency global discovery shines, but Indexers are not needed.

What can we build where a global view is a qualitative advantage, but it is reader-agnostic? Some objective feeds I guess.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZDMPCWN784G0", p.createdAt = 1719050367691;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDMPCWN784G0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZDMP31SBG500"}), (p2:Post {id: "2ZDMPCWN784G0"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZDMZ753WA2G0"}) SET p.content = "Testing event stream", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZDMZ753WA2G0", p.createdAt = 1719055216955000;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDMZ753WA2G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDMZEGPKVK00"}) SET p.content = "Works like a charm", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZDMZEGPKVK00", p.createdAt = 1719055343433999;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDMZEGPKVK00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZDMZ753WA2G0"}), (p2:Post {id: "2ZDMZEGPKVK00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZDMZN6T5RXG0"}) SET p.content = "New event ", p.uri = "pubky:y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pubky.app/posts/2ZDMZN6T5RXG0", p.createdAt = 1719055458384100;
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZDMZN6T5RXG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDN0W875TJ00"}) SET p.content = "Don't despair for the broken timestamp... everything is under control :)", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZDN0W875TJ00", p.createdAt = 1719056129153999;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN0W875TJ00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZDMZN6T5RXG0"}), (p2:Post {id: "2ZDN0W875TJ00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZDN15FY672G0"}) SET p.content = "I guess I should write a bit about why should \"events\" be the unit of data in Pubky.

While Pubky core tries to stay as close to the current web as possible, signed (at some point) events enables low latency broadcasting of changes across the network, for interested parties to fetch, or ignore.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZDN15FY672G0", p.createdAt = 1719056287917000;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN15FY672G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDN19JTA2B00"}) SET p.content = "I am content with adding this capability even at Pubky core level, not because I want Homeserver to support social media directly, but to make them friendly to low-latency/real-time discovery, at least lower the cost of search engines.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZDN19JTA2B00", p.createdAt = 1719056358182000;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN19JTA2B00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZDN15FY672G0"}), (p2:Post {id: "2ZDN19JTA2B00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZDN1KG9QXKG0"}) SET p.content = "Enabling higher level networks of gossiping events + possibly trustless Cache servers, can turn distributed small homeservers into somewhat unified discoverable indexable marketplace, or as John call it matching engines. Never going to be as fast as centralized ones, but might be good enough.", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZDN1KG9QXKG0", p.createdAt = 1719056528629000;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN1KG9QXKG0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZDN19JTA2B00"}), (p2:Post {id: "2ZDN1KG9QXKG0"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZDPGX8W3DTG0"}) SET p.content = "We need more bots to enjoy watching the event stream more!", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZDPGX8W3DTG0", p.createdAt = 1719082534964000;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDPGX8W3DTG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDPHVBK54XG0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 17 sats/vB
🐢 +30 min : 16 sats/vB
🐌 +60 min : 14 sats/vB
🦥 +90 min : 10 sats/vB

🔥 Purge Limit : 5 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDPHVBK54XG0", p.createdAt = 1719083051820596;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDPHVBK54XG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDPQYABQQQ00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 0.20%
$64,295.01

🇧🇷 BTCBRL
🔼 0.19%
R$ 352.123,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDPQYABQQQ00", p.createdAt = 1719086401233743;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDPQYABQQQ00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDPYFYPYTVG0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 9 sats/vB
🐢 +30 min : 9 sats/vB
🐌 +60 min : 9 sats/vB
🦥 +90 min : 9 sats/vB

🔥 Purge Limit : 5 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDPYFYPYTVG0", p.createdAt = 1719090002752092;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDPYFYPYTVG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDQ3D3EKP700"}) SET p.content = "🦾 Bitcoin Difficulty Adjustment

🏁 Current Progress: 364 of 2016 blocks

▓▓▓▓░░░░░░░░░░░░░░░░ 18.06%

🗓️ Estimated Date: 4/7/2024

Current Change   : 🔽 -2.65%
Previous Change : 🔽 -0.05%

#Bitcoin #DifficultyAdjustment #GracePeriod", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDQ3D3EKP700", p.createdAt = 1719092702535847;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQ3D3EKP700"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDQ51DAF22G0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 0.41%
$64,330.00

🇧🇷 BTCBRL
🔼 0.52%
R$ 352.244,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDQ51DAF22G0", p.createdAt = 1719093601188218;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQ51DAF22G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDQ6NZTPNQ00"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 4.33%

⏳ Countdown: 200,898 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 849,102

⏳ Days Until Halving: 1,395 days
🗓️ Halving Date: 17/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDQ6NZTPNQ00", p.createdAt = 1719094504477519;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQ6NZTPNQ00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDQ8A9MQW8G0"}) SET p.content = "⚡ Lightning Network

🪫 Total Capacity: 5,115 BTC
🪫 Avg. Capacity: 10,062,956 sats

🖥️ Total Nodes: 13,028
🤵‍♂️ Clearnet: 1,755
🕵️ Tor: 9,038
🔀 Channels: 50,834

💸 Avg. Fee: 762 ppm
💸 Avg. Base Fee: 949 msats

#Bitcoin #LightningNetwork", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDQ8A9MQW8G0", p.createdAt = 1719095403099394;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQ8A9MQW8G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDQBK17ZQ3G0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 10 sats/vB
🐢 +30 min : 10 sats/vB
🐌 +60 min : 10 sats/vB
🦥 +90 min : 8 sats/vB

🔥 Purge Limit : 4 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDQBK17ZQ3G0", p.createdAt = 1719097202476697;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQBK17ZQ3G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDQJ4GXET7G0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 0.26%
$64,278.56

🇧🇷 BTCBRL
🔼 0.28%
R$ 351.890,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDQJ4GXET7G0", p.createdAt = 1719100801482657;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQJ4GXET7G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDQKS2KMQ000"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 4.34%

⏳ Countdown: 200,892 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 849,108

⏳ Days Until Halving: 1,395 days
🗓️ Halving Date: 17/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDQKS2KMQ000", p.createdAt = 1719101704334852;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQKS2KMQ000"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDQRP4YNY6G0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 9 sats/vB
🐢 +30 min : 9 sats/vB
🐌 +60 min : 9 sats/vB
🦥 +90 min : 8 sats/vB

🔥 Purge Limit : 4 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDQRP4YNY6G0", p.createdAt = 1719104402833247;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQRP4YNY6G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDQZ7KMAVC00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 0.50%
$64,478.68

🇧🇷 BTCBRL
🔼 0.51%
R$ 352.983,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDQZ7KMAVC00", p.createdAt = 1719108001305385;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQZ7KMAVC00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDR5S6HTRQ00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 9 sats/vB
🐢 +30 min : 9 sats/vB
🐌 +60 min : 9 sats/vB
🦥 +90 min : 8 sats/vB

🔥 Purge Limit : 4 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDR5S6HTRQ00", p.createdAt = 1719111602056605;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDR5S6HTRQ00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDRCAPMQAK00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 0.15%
$64,426.73

🇧🇷 BTCBRL
🔼 0.23%
R$ 352.836,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDRCAPMQAK00", p.createdAt = 1719115201287726;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRCAPMQAK00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDRJW8TVK0G0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 9 sats/vB
🐢 +30 min : 9 sats/vB
🐌 +60 min : 9 sats/vB
🦥 +90 min : 6 sats/vB

🔥 Purge Limit : 3 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDRJW8TVK0G0", p.createdAt = 1719118801646961;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRJW8TVK0G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDRSDSNTH400"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -0.02%
$64,380.00

🇧🇷 BTCBRL
🔼 0.03%
R$ 352.561,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDRSDSNTH400", p.createdAt = 1719122401281984;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRSDSNTH400"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDRZZCEDAF00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 18 sats/vB
🐢 +30 min : 14 sats/vB
🐌 +60 min : 12 sats/vB
🦥 +90 min : 6 sats/vB

🔥 Purge Limit : 3 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDRZZCEDAF00", p.createdAt = 1719126001950826;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRZZCEDAF00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDS6GWNSDDG0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -0.14%
$64,415.26

🇧🇷 BTCBRL
🔽 -0.13%
R$ 352.752,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDS6GWNSDDG0", p.createdAt = 1719129601257200;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDS6GWNSDDG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDSBY99RAZ00"}) SET p.content = "Agree. Just didn't think of the following filter. I hope other people will not have the same problem. #KeepPubkySimple", p.uri = "pubky:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao/pubky.app/posts/2ZDSBY99RAZ00", p.createdAt = 1719132580152000;
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2ZDSBY99RAZ00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZD67W27BHB00"}), (p2:Post {id: "2ZDSBY99RAZ00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZDSD2F1V6J00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 9 sats/vB
🐢 +30 min : 9 sats/vB
🐌 +60 min : 9 sats/vB
🦥 +90 min : 6 sats/vB

🔥 Purge Limit : 3 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDSD2F1V6J00", p.createdAt = 1719133201715799;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDSD2F1V6J00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDSKKZWJQJG0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 0.14%
$64,418.80

🇧🇷 BTCBRL
🔼 0.19%
R$ 352.834,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDSKKZWJQJG0", p.createdAt = 1719136801346938;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDSKKZWJQJG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDST5J63VC00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 9 sats/vB
🐢 +30 min : 9 sats/vB
🐌 +60 min : 9 sats/vB
🦥 +90 min : 6 sats/vB

🔥 Purge Limit : 3 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDST5J63VC00", p.createdAt = 1719140401763244;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDST5J63VC00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDT0Q33GBB00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 0.10%
$64,332.43

🇧🇷 BTCBRL
🔼 0.17%
R$ 352.338,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDT0Q33GBB00", p.createdAt = 1719144001438930;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDT0Q33GBB00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDT2BHYHD5G0"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 4.36%

⏳ Countdown: 200,838 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 849,162

⏳ Days Until Halving: 1,395 days
🗓️ Halving Date: 18/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDT2BHYHD5G0", p.createdAt = 1719144902761859;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDT2BHYHD5G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDT34R8J6C00"}) SET p.content = "\"Andy giveth, and Bill taketh away.\"", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZDT34R8J6C00", p.createdAt = 1719145335648000;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDT34R8J6C00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDT5MAJJ55G0"}) SET p.content = "🦾 Bitcoin Difficulty Adjustment

🏁 Current Progress: 426 of 2016 blocks

▓▓▓▓▓░░░░░░░░░░░░░░░ 21.13%

🗓️ Estimated Date: 5/7/2024

Current Change   : 🔽 -6.44%
Previous Change : 🔽 -0.05%

#Bitcoin #DifficultyAdjustment #GracePeriod", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDT5MAJJ55G0", p.createdAt = 1719146702689094;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDT5MAJJ55G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDT78ND8T500"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 18 sats/vB
🐢 +30 min : 15 sats/vB
🐌 +60 min : 12 sats/vB
🦥 +90 min : 8 sats/vB

🔥 Purge Limit : 4 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDT78ND8T500", p.createdAt = 1719147601859086;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDT78ND8T500"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDTAHEV6YR00"}) SET p.content = "⚡ Lightning Network

🪫 Total Capacity: 5,118 BTC
🪫 Avg. Capacity: 10,017,824 sats

🖥️ Total Nodes: 13,151
🤵‍♂️ Clearnet: 1,762
🕵️ Tor: 9,150
🔀 Channels: 51,093

💸 Avg. Fee: 759 ppm
💸 Avg. Base Fee: 947 msats

#Bitcoin #LightningNetwork", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDTAHEV6YR00", p.createdAt = 1719149402221162;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTAHEV6YR00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDTDT694BQG0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔼 0.02%
$64,289.99

🇧🇷 BTCBRL
🔼 0.03%
R$ 352.126,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDTDT694BQG0", p.createdAt = 1719151201509109;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTDT694BQG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDTMBRKJV700"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 19 sats/vB
🐢 +30 min : 16 sats/vB
🐌 +60 min : 13 sats/vB
🦥 +90 min : 10 sats/vB

🔥 Purge Limit : 5 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDTMBRKJV700", p.createdAt = 1719154801940811;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTMBRKJV700"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDTP06PKKTG0"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 4.37%

⏳ Countdown: 200,827 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 849,173

⏳ Days Until Halving: 1,395 days
🗓️ Halving Date: 18/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDTP06PKKTG0", p.createdAt = 1719155702860936;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTP06PKKTG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDTTX9HKAT00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -0.33%
$64,094.00

🇧🇷 BTCBRL
🔽 -0.26%
R$ 351.260,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDTTX9HKAT00", p.createdAt = 1719158401626977;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTTX9HKAT00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDV1EW195M00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 14 sats/vB
🐢 +30 min : 13 sats/vB
🐌 +60 min : 11 sats/vB
🦥 +90 min : 10 sats/vB

🔥 Purge Limit : 5 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDV1EW195M00", p.createdAt = 1719162002146420;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDV1EW195M00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDV33AYH4XG0"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 4.37%

⏳ Countdown: 200,822 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 849,178

⏳ Days Until Halving: 1,395 days
🗓️ Halving Date: 18/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDV33AYH4XG0", p.createdAt = 1719162903506533;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDV33AYH4XG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDV80CDGWC00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -0.21%
$64,122.00

🇧🇷 BTCBRL
🔽 -0.17%
R$ 351.201,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDV80CDGWC00", p.createdAt = 1719165601534384;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDV80CDGWC00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDVEJ08D0Y00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 25 sats/vB
🐢 +30 min : 20 sats/vB
🐌 +60 min : 15 sats/vB
🦥 +90 min : 12 sats/vB

🔥 Purge Limit : 6 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDVEJ08D0Y00", p.createdAt = 1719169202778553;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDVEJ08D0Y00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDVN3FNZHA00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -0.25%
$64,136.66

🇧🇷 BTCBRL
🔽 -0.23%
R$ 351.382,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDVN3FNZHA00", p.createdAt = 1719172801652085;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDVN3FNZHA00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDVVN3JX77G0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 9 sats/vB
🐢 +30 min : 9 sats/vB
🐌 +60 min : 9 sats/vB
🦥 +90 min : 9 sats/vB

🔥 Purge Limit : 5 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDVVN3JX77G0", p.createdAt = 1719176402930618;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDVVN3JX77G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDW0J90AN000"}) SET p.content = "🦾 Bitcoin Difficulty Adjustment

🏁 Current Progress: 470 of 2016 blocks

▓▓▓▓▓░░░░░░░░░░░░░░░ 23.31%

🗓️ Estimated Date: 5/7/2024

Current Change   : 🔽 -6.00%
Previous Change : 🔽 -0.05%

#Bitcoin #DifficultyAdjustment #GracePeriod", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDW0J90AN000", p.createdAt = 1719179103079579;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW0J90AN000"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDW26N6BG3G0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -0.77%
$63,837.68

🇧🇷 BTCBRL
🔽 -0.74%
R$ 349.625,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDW26N6BG3G0", p.createdAt = 1719180002976335;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW26N6BG3G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDW3V5Y7W000"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 4.39%

⏳ Countdown: 200,790 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 849,210

⏳ Days Until Halving: 1,394 days
🗓️ Halving Date: 18/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDW3V5Y7W000", p.createdAt = 1719180905320218;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW3V5Y7W000"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDW5FEHH6C00"}) SET p.content = "⚡ Lightning Network

🪫 Total Capacity: 5,126 BTC
🪫 Avg. Capacity: 10,036,013 sats

🖥️ Total Nodes: 13,087
🤵‍♂️ Clearnet: 1,762
🕵️ Tor: 9,084
🔀 Channels: 51,076

💸 Avg. Fee: 761 ppm
💸 Avg. Base Fee: 947 msats

#Bitcoin #LightningNetwork", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDW5FEHH6C00", p.createdAt = 1719181803292039;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW5FEHH6C00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDW8R72ANMG0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 10 sats/vB
🐢 +30 min : 10 sats/vB
🐌 +60 min : 9 sats/vB
🦥 +90 min : 8 sats/vB

🔥 Purge Limit : 4 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDW8R72ANMG0", p.createdAt = 1719183603165129;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW8R72ANMG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDWF9PVQ4SG0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -1.68%
$63,202.01

🇧🇷 BTCBRL
🔽 -1.60%
R$ 346.250,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDWF9PVQ4SG0", p.createdAt = 1719187202236822;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDWF9PVQ4SG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDWGY905Z4G0"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 4.39%

⏳ Countdown: 200,778 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 849,222

⏳ Days Until Halving: 1,394 days
🗓️ Halving Date: 18/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDWGY905Z4G0", p.createdAt = 1719188105328576;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDWGY905Z4G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDWNV9TDMT00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 9 sats/vB
🐢 +30 min : 9 sats/vB
🐌 +60 min : 9 sats/vB
🦥 +90 min : 8 sats/vB

🔥 Purge Limit : 4 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDWNV9TDMT00", p.createdAt = 1719190803008271;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDWNV9TDMT00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDWWCSRRZJG0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -1.79%
$63,322.01

🇧🇷 BTCBRL
🔽 -1.76%
R$ 346.744,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDWWCSRRZJG0", p.createdAt = 1719194402163254;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDWWCSRRZJG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDX2YBC5Y7G0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 9 sats/vB
🐢 +30 min : 9 sats/vB
🐌 +60 min : 9 sats/vB
🦥 +90 min : 8 sats/vB

🔥 Purge Limit : 4 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDX2YBC5Y7G0", p.createdAt = 1719198002208280;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDX2YBC5Y7G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDX9FWEXSW00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -2.43%
$62,863.49

🇧🇷 BTCBRL
🔽 -2.38%
R$ 344.439,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDX9FWEXSW00", p.createdAt = 1719201601973811;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDX9FWEXSW00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDXG1EB5TZ00"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 9 sats/vB
🐢 +30 min : 9 sats/vB
🐌 +60 min : 9 sats/vB
🦥 +90 min : 6 sats/vB

🔥 Purge Limit : 3 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDXG1EB5TZ00", p.createdAt = 1719205202167250;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDXG1EB5TZ00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDXPJZ3SB600"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -3.15%
$62,352.01

🇧🇷 BTCBRL
🔽 -3.12%
R$ 341.566,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDXPJZ3SB600", p.createdAt = 1719208801762724;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDXPJZ3SB600"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDXX4HE9DPG0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 10 sats/vB
🐢 +30 min : 9 sats/vB
🐌 +60 min : 9 sats/vB
🦥 +90 min : 6 sats/vB

🔥 Purge Limit : 3 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDXX4HE9DPG0", p.createdAt = 1719212402195262;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDXX4HE9DPG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDY3P2P13RG0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -3.06%
$62,443.51

🇧🇷 BTCBRL
🔽 -3.05%
R$ 341.988,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDY3P2P13RG0", p.createdAt = 1719216002044588;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDY3P2P13RG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDYA7MH312G0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 12 sats/vB
🐢 +30 min : 11 sats/vB
🐌 +60 min : 10 sats/vB
🦥 +90 min : 6 sats/vB

🔥 Purge Limit : 3 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDYA7MH312G0", p.createdAt = 1719219602218042;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYA7MH312G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDYGS5S86D00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -4.70%
$61,390.00

🇧🇷 BTCBRL
🔽 -4.66%
R$ 336.395,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDYGS5S86D00", p.createdAt = 1719223202074434;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYGS5S86D00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDYQAQFA74G0"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 12 sats/vB
🐢 +30 min : 12 sats/vB
🐌 +60 min : 11 sats/vB
🦥 +90 min : 6 sats/vB

🔥 Purge Limit : 3 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDYQAQFA74G0", p.createdAt = 1719226802164058;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYQAQFA74G0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDYXW8751NG0"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -4.74%
$61,282.01

🇧🇷 BTCBRL
🔽 -4.65%
R$ 335.974,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDYXW8751NG0", p.createdAt = 1719230401746594;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYXW8751NG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDYZGQ4XKTG0"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 4.43%

⏳ Countdown: 200,704 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 849,296

⏳ Days Until Halving: 1,394 days
🗓️ Halving Date: 18/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDYZGQ4XKTG0", p.createdAt = 1719231303115404;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYZGQ4XKTG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDZ2SF29HK00"}) SET p.content = "🦾 Bitcoin Difficulty Adjustment

🏁 Current Progress: 563 of 2016 blocks

▓▓▓▓▓▓░░░░░░░░░░░░░░ 27.93%

🗓️ Estimated Date: 5/7/2024

Current Change   : 🔽 -5.29%
Previous Change : 🔽 -0.05%

#Bitcoin #DifficultyAdjustment #GracePeriod", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDZ2SF29HK00", p.createdAt = 1719233102662624;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZ2SF29HK00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDZ4DTKRJ900"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 9 sats/vB
🐢 +30 min : 9 sats/vB
🐌 +60 min : 9 sats/vB
🦥 +90 min : 8 sats/vB

🔥 Purge Limit : 4 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDZ4DTKRJ900", p.createdAt = 1719234002214490;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZ4DTKRJ900"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDZ7PM0JVK00"}) SET p.content = "⚡ Lightning Network

🪫 Total Capacity: 5,141 BTC
🪫 Avg. Capacity: 10,064,375 sats

🖥️ Total Nodes: 13,095
🤵‍♂️ Clearnet: 1,761
🕵️ Tor: 9,101
🔀 Channels: 51,085

💸 Avg. Fee: 761 ppm
💸 Avg. Base Fee: 950 msats

#Bitcoin #LightningNetwork", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDZ7PM0JVK00", p.createdAt = 1719235802557769;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZ7PM0JVK00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDZ87NZ4H700"}) SET p.content = "https://www.youtube.com/watch?v=8j4fhsLcT4k", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/2ZDZ87NZ4H700", p.createdAt = 1719236095665000;
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDZ87NZ4H700"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDZHGVTQV600"}) SET p.content = "💸 Bitcoin Fees

🐇 Fastest : 13 sats/vB
🐢 +30 min : 11 sats/vB
🐌 +60 min : 10 sats/vB
🦥 +90 min : 6 sats/vB

🔥 Purge Limit : 3 sats/vB

#Bitcoin #fees", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDZHGVTQV600", p.createdAt = 1719241201233611;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZHGVTQV600"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDZK595DDRG0"}) SET p.content = "🎉 Bitcoin Halving Countdown

▓░░░░░░░░░░░░░░░░░░░ 4.44%

⏳ Countdown: 200,679 Blocks

🔗 Next Halving Block: 1,050,000
🔗 Current Block: 849,321

⏳ Days Until Halving: 1,394 days
🗓️ Halving Date: 18/4/2028
📅 Next Halving Era: 5

#Bitcoin #Halving", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDZK595DDRG0", p.createdAt = 1719242101745217;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZK595DDRG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZDZR2G775W00"}) SET p.content = "💵 Bitcoin Fiat Price

🇺🇸 BTCUSDT
🔽 -5.07%
$60,843.63

🇧🇷 BTCBRL
🔽 -5.81%
R$ 330.784,00

#Bitcoin #price", p.uri = "pubky:kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o/pubky.app/posts/2ZDZR2G775W00", p.createdAt = 1719244802773055;
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZR2G775W00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZE8P6VPBVW00"}) SET p.content = "#Bitkit
https://x.com/bitkitwallet/status/1801242110974382468", p.uri = "pubky:h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy/pubky.app/posts/2ZE8P6VPBVW00", p.createdAt = 1719402107815000;
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZE8P6VPBVW00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZEBH4J0K4G00"}) SET p.content = "1", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZEBH4J0K4G00", p.createdAt = 1719452096037199;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZEBH4J0K4G00"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZECRNM66G900"}) SET p.content = "🇮🇹👀
https://x.com/paoloardoino/status/1805349838533570754", p.uri = "pubky:h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy/pubky.app/posts/2ZECRNM66G900", p.createdAt = 1719473829739500;
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZECXVXHZBE00"}) SET p.content = "@Sev Number 3 pk:kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao

Saturday 29/06/2023, 18:00
🇨🇭 Swiss VS 🇮🇹 Italy", p.uri = "pubky:h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy/pubky.app/posts/2ZECXVXHZBE00", p.createdAt = 1719476686627200;
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECXVXHZBE00"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (p1:Post {id: "2ZECRNM66G900"}), (p2:Post {id: "2ZECXVXHZBE00"}) MERGE (p2)-[:REPLY_TO]->(p1);
MERGE (p:Post {id: "2ZEEM0CKE3CG0"}) SET p.content = "LET'SSSS GOOOOOOOOOOOOOOOOO!", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZEEM0CKE3CG0", p.createdAt = 1719506450238;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZEEM0CKE3CG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZEHQ1G68GXG0"}) SET p.content = "test", p.uri = "pubky:sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy/pubky.app/posts/2ZEHQ1G68GXG0", p.createdAt = 1719560895170;
MATCH (u:User {id: "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy"}), (p:Post {id: "2ZEHQ1G68GXG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZF76EAN4C900"}) SET p.content = "test post", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZF76EAN4C900", p.createdAt = 1719938797780;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZF76EAN4C900"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZF76JGZMEHG0"}) SET p.content = "test 2", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZF76JGZMEHG0", p.createdAt = 1719938869897;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZF76JGZMEHG0"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "2ZF7PFV56HRG0"}) SET p.content = "https://github.com/synonymdev/bitkit", p.uri = "pubky:o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo/pubky.app/posts/2ZF7PFV56HRG0", p.createdAt = 1719947619913;
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZF7PFV56HRG0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719913595668}]->(u2);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719913597757}]->(u2);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712311528319}]->(u2);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719913697162}]->(u2);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712322486017}]->(u2);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719913603944}]->(u2);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719913602578}]->(u2);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712311592664}]->(u2);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719488060516}]->(u2);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "kg671gqu6akiyuzdsqgqtupftuhbuwfx5zh6tbygmexuw6b55s4y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719913612358}]->(u2);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719913614043}]->(u2);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719488061126}]->(u2);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712311131848}]->(u2);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719913679102}]->(u2);
MATCH (u1:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (u2:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719913624762}]->(u2);
MATCH (u1:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (u2:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533601}]->(u2);
MATCH (u1:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (u2:User {id: "xgoium16biftc8z7ecrbwrdokdwr1xphdke4nuaqiutnihdmxmny"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533602}]->(u2);
MATCH (u1:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (u2:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533602}]->(u2);
MATCH (u1:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (u2:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533602}]->(u2);
MATCH (u1:User {id: "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713342792482}]->(u2);
MATCH (u1:User {id: "77m8jyqqrd41xzu5b67m8z5wuypuatc54gmw5gcax6ae3yeca6wo"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719904166247}]->(u2);
MATCH (u1:User {id: "77m8jyqqrd41xzu5b67m8z5wuypuatc54gmw5gcax6ae3yeca6wo"}), (u2:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719904454628}]->(u2);
MATCH (u1:User {id: "77m8jyqqrd41xzu5b67m8z5wuypuatc54gmw5gcax6ae3yeca6wo"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719904165599}]->(u2);
MATCH (u1:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (u2:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042906}]->(u2);
MATCH (u1:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (u2:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042904}]->(u2);
MATCH (u1:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (u2:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042904}]->(u2);
MATCH (u1:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (u2:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042804}]->(u2);
MATCH (u1:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (u2:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042813}]->(u2);
MATCH (u1:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (u2:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042810}]->(u2);
MATCH (u1:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (u2:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042808}]->(u2);
MATCH (u1:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (u2:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042805}]->(u2);
MATCH (u1:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (u2:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533647}]->(u2);
MATCH (u1:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (u2:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533648}]->(u2);
MATCH (u1:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (u2:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533648}]->(u2);
MATCH (u1:User {id: "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y"}), (u2:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717574464513}]->(u2);
MATCH (u1:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (u2:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533626}]->(u2);
MATCH (u1:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (u2:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533626}]->(u2);
MATCH (u1:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (u2:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533627}]->(u2);
MATCH (u1:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (u2:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533627}]->(u2);
MATCH (u1:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (u2:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712312059570}]->(u2);
MATCH (u1:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (u2:User {id: "end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712312103108}]->(u2);
MATCH (u1:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712321561573}]->(u2);
MATCH (u1:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (u2:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712321565989}]->(u2);
MATCH (u1:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (u2:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712312119274}]->(u2);
MATCH (u1:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (u2:User {id: "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712312087633}]->(u2);
MATCH (u1:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712322343247}]->(u2);
MATCH (u1:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (u2:User {id: "sse599pgxr9ahwrrns7yz3dpd118wjjpiisscrh4qnrgj4334iuo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712322191072}]->(u2);
MATCH (u1:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (u2:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712312029905}]->(u2);
MATCH (u1:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (u2:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712322157703}]->(u2);
MATCH (u1:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (u2:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042851}]->(u2);
MATCH (u1:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (u2:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042850}]->(u2);
MATCH (u1:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (u2:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042849}]->(u2);
MATCH (u1:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (u2:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533619}]->(u2);
MATCH (u1:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (u2:User {id: "xgoium16biftc8z7ecrbwrdokdwr1xphdke4nuaqiutnihdmxmny"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533619}]->(u2);
MATCH (u1:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (u2:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533621}]->(u2);
MATCH (u1:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (u2:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533620}]->(u2);
MATCH (u1:User {id: "dujh8de33hbaqerg1ejyu9ynjh5yqfozm6iexsjdj3h9yfn3n3wy"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1714037847429}]->(u2);
MATCH (u1:User {id: "dujh8de33hbaqerg1ejyu9ynjh5yqfozm6iexsjdj3h9yfn3n3wy"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1714037844585}]->(u2);
MATCH (u1:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (u2:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533634}]->(u2);
MATCH (u1:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (u2:User {id: "xgoium16biftc8z7ecrbwrdokdwr1xphdke4nuaqiutnihdmxmny"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533632}]->(u2);
MATCH (u1:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (u2:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533634}]->(u2);
MATCH (u1:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (u2:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533633}]->(u2);
MATCH (u1:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (u2:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716367949012}]->(u2);
MATCH (u1:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (u2:User {id: "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716367949862}]->(u2);
MATCH (u1:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (u2:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716367952993}]->(u2);
MATCH (u1:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (u2:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716367956463}]->(u2);
MATCH (u1:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716367522815}]->(u2);
MATCH (u1:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (u2:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716367957876}]->(u2);
MATCH (u1:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (u2:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719478343588}]->(u2);
MATCH (u1:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (u2:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716367941379}]->(u2);
MATCH (u1:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716367521267}]->(u2);
MATCH (u1:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (u2:User {id: "sse599pgxr9ahwrrns7yz3dpd118wjjpiisscrh4qnrgj4334iuo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716367962499}]->(u2);
MATCH (u1:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (u2:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716367963822}]->(u2);
MATCH (u1:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (u2:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716367964863}]->(u2);
MATCH (u1:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (u2:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719212724925}]->(u2);
MATCH (u1:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (u2:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717142199485}]->(u2);
MATCH (u1:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (u2:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719212600817}]->(u2);
MATCH (u1:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713966513804}]->(u2);
MATCH (u1:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (u2:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719913731037}]->(u2);
MATCH (u1:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (u2:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719212791192}]->(u2);
MATCH (u1:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (u2:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719212793253}]->(u2);
MATCH (u1:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (u2:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1714658527741}]->(u2);
MATCH (u1:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (u2:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719496014672}]->(u2);
MATCH (u1:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (u2:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718715470955}]->(u2);
MATCH (u1:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719061167181}]->(u2);
MATCH (u1:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (u2:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1715413157534}]->(u2);
MATCH (u1:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (u2:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718882879175}]->(u2);
MATCH (u1:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (u2:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712311226607}]->(u2);
MATCH (u1:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (u2:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712311353256}]->(u2);
MATCH (u1:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (u2:User {id: "end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712311333540}]->(u2);
MATCH (u1:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (u2:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712310097194}]->(u2);
MATCH (u1:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (u2:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712311239186}]->(u2);
MATCH (u1:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (u2:User {id: "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712310081762}]->(u2);
MATCH (u1:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712303174810}]->(u2);
MATCH (u1:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (u2:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712311277555}]->(u2);
MATCH (u1:User {id: "hrrrixk98mzi89c99m9a1e53fxszb6f44nzz3rptt5n6r5j4pxco"}), (u2:User {id: "mwpzjenysim7koioqc8qf4ymwgpyi6eotnr9pxuuskh5qndcngzy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1720195542994}]->(u2);
MATCH (u1:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (u2:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719909640740}]->(u2);
MATCH (u1:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (u2:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719909611289}]->(u2);
MATCH (u1:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (u2:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1720085820508}]->(u2);
MATCH (u1:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (u2:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}) MERGE (u1)-[:FOLLOWS {createdAt: 1720085435406}]->(u2);
MATCH (u1:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (u2:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1720085436366}]->(u2);
MATCH (u1:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719909581338}]->(u2);
MATCH (u1:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (u2:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719909637937}]->(u2);
MATCH (u1:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (u2:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718286829496}]->(u2);
MATCH (u1:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (u2:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716812925173}]->(u2);
MATCH (u1:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (u2:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718286833695}]->(u2);
MATCH (u1:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718286895095}]->(u2);
MATCH (u1:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (u2:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718286768277}]->(u2);
MATCH (u1:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712321436749}]->(u2);
MATCH (u1:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712311373088}]->(u2);
MATCH (u1:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (u2:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042880}]->(u2);
MATCH (u1:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (u2:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042879}]->(u2);
MATCH (u1:User {id: "jn6rpfszntneom7tx5aj4kcn4rid5adfwtsusog5yunpn1ptp86o"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712821057596}]->(u2);
MATCH (u1:User {id: "kg671gqu6akiyuzdsqgqtupftuhbuwfx5zh6tbygmexuw6b55s4y"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1715868684496}]->(u2);
MATCH (u1:User {id: "kg671gqu6akiyuzdsqgqtupftuhbuwfx5zh6tbygmexuw6b55s4y"}), (u2:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1715868597954}]->(u2);
MATCH (u1:User {id: "kg671gqu6akiyuzdsqgqtupftuhbuwfx5zh6tbygmexuw6b55s4y"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1715868603896}]->(u2);
MATCH (u1:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (u2:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719225139005}]->(u2);
MATCH (u1:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (u2:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718780138951}]->(u2);
MATCH (u1:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (u2:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719132532199}]->(u2);
MATCH (u1:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718612312080}]->(u2);
MATCH (u1:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (u2:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719132517123}]->(u2);
MATCH (u1:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (u2:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718780135189}]->(u2);
MATCH (u1:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (u2:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719132893209}]->(u2);
MATCH (u1:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (u2:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719225138414}]->(u2);
MATCH (u1:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (u2:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718779341622}]->(u2);
MATCH (u1:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717568394047}]->(u2);
MATCH (u1:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (u2:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718780149472}]->(u2);
MATCH (u1:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (u2:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719132892528}]->(u2);
MATCH (u1:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (u2:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718779343481}]->(u2);
MATCH (u1:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (u2:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719132524461}]->(u2);
MATCH (u1:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (u2:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042870}]->(u2);
MATCH (u1:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (u2:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042872}]->(u2);
MATCH (u1:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (u2:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042869}]->(u2);
MATCH (u1:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (u2:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042870}]->(u2);
MATCH (u1:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (u2:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533641}]->(u2);
MATCH (u1:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (u2:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533641}]->(u2);
MATCH (u1:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (u2:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533641}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713368507873}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713441705443}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713389873813}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713368404789}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "fh961bofrue6oqupmu6xdqn31sghfjxkefq7ocsf9osub7jn7r9y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719328023840}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716382008463}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1715732122741}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718984496089}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716228237488}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713441693315}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713441689046}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "kg671gqu6akiyuzdsqgqtupftuhbuwfx5zh6tbygmexuw6b55s4y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716382003928}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716813206292}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713368510401}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713406159069}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718277730195}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "sse599pgxr9ahwrrns7yz3dpd118wjjpiisscrh4qnrgj4334iuo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713368512569}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713368517113}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713368391045}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713441701563}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "ym1rn4rfjg8857y13uuehc4tw6sfqtzpu881ycdj7iiyo6kqsspy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717600050978}]->(u2);
MATCH (u1:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (u2:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717039961301}]->(u2);
MATCH (u1:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (u2:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042910}]->(u2);
MATCH (u1:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (u2:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042911}]->(u2);
MATCH (u1:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (u2:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042912}]->(u2);
MATCH (u1:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (u2:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042912}]->(u2);
MATCH (u1:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (u2:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042911}]->(u2);
MATCH (u1:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712321124442}]->(u2);
MATCH (u1:User {id: "pcckx7sercfy1u8rrr8cc4gkdnce93f6jarngcdsfu5enty51aiy"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716812462243}]->(u2);
MATCH (u1:User {id: "pcckx7sercfy1u8rrr8cc4gkdnce93f6jarngcdsfu5enty51aiy"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716812461346}]->(u2);
MATCH (u1:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (u2:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042860}]->(u2);
MATCH (u1:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (u2:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042861}]->(u2);
MATCH (u1:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (u2:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042859}]->(u2);
MATCH (u1:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (u2:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042897}]->(u2);
MATCH (u1:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (u2:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042897}]->(u2);
MATCH (u1:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (u2:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042896}]->(u2);
MATCH (u1:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (u2:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042898}]->(u2);
MATCH (u1:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (u2:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042898}]->(u2);
MATCH (u1:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (u2:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042887}]->(u2);
MATCH (u1:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (u2:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042887}]->(u2);
MATCH (u1:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (u2:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042889}]->(u2);
MATCH (u1:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (u2:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042888}]->(u2);
MATCH (u1:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (u2:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042888}]->(u2);
MATCH (u1:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (u2:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716459107703}]->(u2);
MATCH (u1:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713288868255}]->(u2);
MATCH (u1:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (u2:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712309850540}]->(u2);
MATCH (u1:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (u2:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713288859322}]->(u2);
MATCH (u1:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (u2:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717447083647}]->(u2);
MATCH (u1:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (u2:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717474171994}]->(u2);
MATCH (u1:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (u2:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718304487058}]->(u2);
MATCH (u1:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (u2:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718304489411}]->(u2);
MATCH (u1:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718304456693}]->(u2);
MATCH (u1:User {id: "sse599pgxr9ahwrrns7yz3dpd118wjjpiisscrh4qnrgj4334iuo"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712324851696}]->(u2);
MATCH (u1:User {id: "sse599pgxr9ahwrrns7yz3dpd118wjjpiisscrh4qnrgj4334iuo"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712321812671}]->(u2);
MATCH (u1:User {id: "uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho"}), (u2:User {id: "7oq5wj1adxk1u94ojh6eknwj3b4z88zcbb51dbam5zn7zeqnzoio"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719983435614}]->(u2);
MATCH (u1:User {id: "uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719933752458}]->(u2);
MATCH (u1:User {id: "uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719933753054}]->(u2);
MATCH (u1:User {id: "uudfeafc1c6dhxxnaiyuzsy3yy79i1ikpb8syht46qpnx4ksi6ho"}), (u2:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719933779628}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713364011527}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1715608651137}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713364017379}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713238402257}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713364021607}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713364053711}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713364049195}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713364045062}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713406777276}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713236454787}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "sse599pgxr9ahwrrns7yz3dpd118wjjpiisscrh4qnrgj4334iuo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713364036799}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713236394107}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713364029519}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1715608288238}]->(u2);
MATCH (u1:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (u2:User {id: "ze86rtgp6x1qdyno4uzp8gexbb887dtemmonoh4j3iisbzitcppo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713364035655}]->(u2);
MATCH (u1:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (u2:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042835}]->(u2);
MATCH (u1:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (u2:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042833}]->(u2);
MATCH (u1:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (u2:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042836}]->(u2);
MATCH (u1:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (u2:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305042831}]->(u2);
MATCH (u1:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}), (u2:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712321972851}]->(u2);
MATCH (u1:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712322517275}]->(u2);
MATCH (u1:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}), (u2:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712322497339}]->(u2);
MATCH (u1:User {id: "xgoium16biftc8z7ecrbwrdokdwr1xphdke4nuaqiutnihdmxmny"}), (u2:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533594}]->(u2);
MATCH (u1:User {id: "xgoium16biftc8z7ecrbwrdokdwr1xphdke4nuaqiutnihdmxmny"}), (u2:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533594}]->(u2);
MATCH (u1:User {id: "xgoium16biftc8z7ecrbwrdokdwr1xphdke4nuaqiutnihdmxmny"}), (u2:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533593}]->(u2);
MATCH (u1:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (u2:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533586}]->(u2);
MATCH (u1:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (u2:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533584}]->(u2);
MATCH (u1:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (u2:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533585}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717752741700}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717752743594}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716277077897}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717752748396}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717752752305}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717752753072}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717752754705}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717752756338}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "jn6rpfszntneom7tx5aj4kcn4rid5adfwtsusog5yunpn1ptp86o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717752758378}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "kg671gqu6akiyuzdsqgqtupftuhbuwfx5zh6tbygmexuw6b55s4y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717752759159}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717752761141}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716277079383}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "pcckx7sercfy1u8rrr8cc4gkdnce93f6jarngcdsfu5enty51aiy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717752785735}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1713334539362}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "sse599pgxr9ahwrrns7yz3dpd118wjjpiisscrh4qnrgj4334iuo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717752784690}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716277085772}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716277084333}]->(u2);
MATCH (u1:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (u2:User {id: "ze86rtgp6x1qdyno4uzp8gexbb887dtemmonoh4j3iisbzitcppo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717752804885}]->(u2);
MATCH (u1:User {id: "ym1rn4rfjg8857y13uuehc4tw6sfqtzpu881ycdj7iiyo6kqsspy"}), (u2:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}) MERGE (u1)-[:FOLLOWS {createdAt: 1717599164951}]->(u2);
MATCH (u1:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (u2:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}) MERGE (u1)-[:FOLLOWS {createdAt: 1719496987614}]->(u2);
MATCH (u1:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (u2:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716819136324}]->(u2);
MATCH (u1:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (u2:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718787609494}]->(u2);
MATCH (u1:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (u2:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716818865290}]->(u2);
MATCH (u1:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (u2:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718799052782}]->(u2);
MATCH (u1:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (u2:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718799054870}]->(u2);
MATCH (u1:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716818956991}]->(u2);
MATCH (u1:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (u2:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1716819135133}]->(u2);
MATCH (u1:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (u2:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1718787820613}]->(u2);
MATCH (u1:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (u2:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533575}]->(u2);
MATCH (u1:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (u2:User {id: "xgoium16biftc8z7ecrbwrdokdwr1xphdke4nuaqiutnihdmxmny"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533573}]->(u2);
MATCH (u1:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (u2:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533576}]->(u2);
MATCH (u1:User {id: "ze86rtgp6x1qdyno4uzp8gexbb887dtemmonoh4j3iisbzitcppo"}), (u2:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712321494763}]->(u2);
MATCH (u1:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (u2:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533611}]->(u2);
MATCH (u1:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (u2:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533613}]->(u2);
MATCH (u1:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (u2:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533612}]->(u2);
MATCH (u1:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (u2:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}) MERGE (u1)-[:FOLLOWS {createdAt: 1712305533614}]->(u2);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "0RDVFKFBB48G"}) MERGE (u)-[:TAGGED {tag: "🔥"}]->(p);
MATCH (u:User {id: "77m8jyqqrd41xzu5b67m8z5wuypuatc54gmw5gcax6ae3yeca6wo"}), (p:Post {id: "0RE3WS2NPCP0"}) MERGE (u)-[:TAGGED {tag: "hello"}]->(p);
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RE3ZCC1Z0KG"}) MERGE (u)-[:TAGGED {tag: "bitkit"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1N38NX16P00"}) MERGE (u)-[:TAGGED {tag: "a third tag"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1N38NX16P00"}) MERGE (u)-[:TAGGED {tag: "first"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1N38NX16P00"}) MERGE (u)-[:TAGGED {tag: "first"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1N38NX16P00"}) MERGE (u)-[:TAGGED {tag: "hello"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1N38NX16P00"}) MERGE (u)-[:TAGGED {tag: "test"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBER6300"}) MERGE (u)-[:TAGGED {tag: "cardigan"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBER6300"}) MERGE (u)-[:TAGGED {tag: "cardigan"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBER6300"}) MERGE (u)-[:TAGGED {tag: "hungrily"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBER6300"}) MERGE (u)-[:TAGGED {tag: "yet"}]->(p);
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBERAVG0"}) MERGE (u)-[:TAGGED {tag: "anti"}]->(p);
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBERAVG0"}) MERGE (u)-[:TAGGED {tag: "beside"}]->(p);
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBERAVG0"}) MERGE (u)-[:TAGGED {tag: "catalogue"}]->(p);
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBERAVG0"}) MERGE (u)-[:TAGGED {tag: "gosh"}]->(p);
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBERAVG0"}) MERGE (u)-[:TAGGED {tag: "yum"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBERD800"}) MERGE (u)-[:TAGGED {tag: "anti"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBERD800"}) MERGE (u)-[:TAGGED {tag: "establishment"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBERD800"}) MERGE (u)-[:TAGGED {tag: "gosh"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBERD800"}) MERGE (u)-[:TAGGED {tag: "instantly"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBERD800"}) MERGE (u)-[:TAGGED {tag: "under"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBERF700"}) MERGE (u)-[:TAGGED {tag: "anti"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBERF700"}) MERGE (u)-[:TAGGED {tag: "enthuse"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBERF700"}) MERGE (u)-[:TAGGED {tag: "huzzah"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBERF700"}) MERGE (u)-[:TAGGED {tag: "innocent"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBERF700"}) MERGE (u)-[:TAGGED {tag: "manicure"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBERF700"}) MERGE (u)-[:TAGGED {tag: "partially"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBERF700"}) MERGE (u)-[:TAGGED {tag: "yum"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERHB00"}) MERGE (u)-[:TAGGED {tag: "buckle"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERHB00"}) MERGE (u)-[:TAGGED {tag: "catalogue"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERHB00"}) MERGE (u)-[:TAGGED {tag: "deeply"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERHB00"}) MERGE (u)-[:TAGGED {tag: "including"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERHB00"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERHB00"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERHB00"}) MERGE (u)-[:TAGGED {tag: "preserve"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERHB00"}) MERGE (u)-[:TAGGED {tag: "psst"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERHB00"}) MERGE (u)-[:TAGGED {tag: "rule"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERJQ00"}) MERGE (u)-[:TAGGED {tag: "as"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERJQ00"}) MERGE (u)-[:TAGGED {tag: "establishment"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERJQ00"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERJQ00"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBERMMG0"}) MERGE (u)-[:TAGGED {tag: "beside"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBERMMG0"}) MERGE (u)-[:TAGGED {tag: "flip"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBERMMG0"}) MERGE (u)-[:TAGGED {tag: "flip"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBERMMG0"}) MERGE (u)-[:TAGGED {tag: "inasmuch"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBERMMG0"}) MERGE (u)-[:TAGGED {tag: "innocent"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBERMMG0"}) MERGE (u)-[:TAGGED {tag: "psst"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBERMMG0"}) MERGE (u)-[:TAGGED {tag: "rule"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBERNYG0"}) MERGE (u)-[:TAGGED {tag: "as"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBERNYG0"}) MERGE (u)-[:TAGGED {tag: "cardigan"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBERNYG0"}) MERGE (u)-[:TAGGED {tag: "deeply"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBERNYG0"}) MERGE (u)-[:TAGGED {tag: "quirkily"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBERNYG0"}) MERGE (u)-[:TAGGED {tag: "yearningly"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBERNYG0"}) MERGE (u)-[:TAGGED {tag: "yearningly"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBERNYG0"}) MERGE (u)-[:TAGGED {tag: "yum"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBERQ9G0"}) MERGE (u)-[:TAGGED {tag: "forecast"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBERQ9G0"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBERQ9G0"}) MERGE (u)-[:TAGGED {tag: "opportunity"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBERQ9G0"}) MERGE (u)-[:TAGGED {tag: "rule"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBERQ9G0"}) MERGE (u)-[:TAGGED {tag: "yet"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERS9G0"}) MERGE (u)-[:TAGGED {tag: "but"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERS9G0"}) MERGE (u)-[:TAGGED {tag: "drummer"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERS9G0"}) MERGE (u)-[:TAGGED {tag: "forecast"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERS9G0"}) MERGE (u)-[:TAGGED {tag: "mastication"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERS9G0"}) MERGE (u)-[:TAGGED {tag: "preserve"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERS9G0"}) MERGE (u)-[:TAGGED {tag: "rule"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBERS9G0"}) MERGE (u)-[:TAGGED {tag: "wealthy"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBERTQG0"}) MERGE (u)-[:TAGGED {tag: "cardigan"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBERTQG0"}) MERGE (u)-[:TAGGED {tag: "catalogue"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBERTQG0"}) MERGE (u)-[:TAGGED {tag: "psst"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBERTQG0"}) MERGE (u)-[:TAGGED {tag: "satisfied"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBERTQG0"}) MERGE (u)-[:TAGGED {tag: "than"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBERTQG0"}) MERGE (u)-[:TAGGED {tag: "within"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBERWS00"}) MERGE (u)-[:TAGGED {tag: "deeply"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBERWS00"}) MERGE (u)-[:TAGGED {tag: "forecast"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBERWS00"}) MERGE (u)-[:TAGGED {tag: "legume"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBERWS00"}) MERGE (u)-[:TAGGED {tag: "less"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBERWS00"}) MERGE (u)-[:TAGGED {tag: "satisfied"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESAF00"}) MERGE (u)-[:TAGGED {tag: "behind"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESAF00"}) MERGE (u)-[:TAGGED {tag: "establishment"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESAF00"}) MERGE (u)-[:TAGGED {tag: "gosh"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESAF00"}) MERGE (u)-[:TAGGED {tag: "including"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESAF00"}) MERGE (u)-[:TAGGED {tag: "inevitable"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESAF00"}) MERGE (u)-[:TAGGED {tag: "partially"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESAF00"}) MERGE (u)-[:TAGGED {tag: "psst"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESAF00"}) MERGE (u)-[:TAGGED {tag: "yearningly"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESAF00"}) MERGE (u)-[:TAGGED {tag: "yum"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESBWG0"}) MERGE (u)-[:TAGGED {tag: "denominator"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESBWG0"}) MERGE (u)-[:TAGGED {tag: "offensively"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESBWG0"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBESDD00"}) MERGE (u)-[:TAGGED {tag: "about"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBESDD00"}) MERGE (u)-[:TAGGED {tag: "including"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBESDD00"}) MERGE (u)-[:TAGGED {tag: "instantly"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBESDD00"}) MERGE (u)-[:TAGGED {tag: "manicure"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBESDD00"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBESDD00"}) MERGE (u)-[:TAGGED {tag: "opportunity"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESER00"}) MERGE (u)-[:TAGGED {tag: "flip"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESER00"}) MERGE (u)-[:TAGGED {tag: "how"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESER00"}) MERGE (u)-[:TAGGED {tag: "including"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESER00"}) MERGE (u)-[:TAGGED {tag: "knavishly"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESER00"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESER00"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESER00"}) MERGE (u)-[:TAGGED {tag: "satisfied"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESER00"}) MERGE (u)-[:TAGGED {tag: "sentimental"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESER00"}) MERGE (u)-[:TAGGED {tag: "sentimental"}]->(p);
MATCH (u:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}), (p:Post {id: "2Z1N8QBESER00"}) MERGE (u)-[:TAGGED {tag: "wealthy"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBESGGG0"}) MERGE (u)-[:TAGGED {tag: "flip"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBESGGG0"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBESKYG0"}) MERGE (u)-[:TAGGED {tag: "drummer"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBESKYG0"}) MERGE (u)-[:TAGGED {tag: "inasmuch"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBESKYG0"}) MERGE (u)-[:TAGGED {tag: "innocent"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBESKYG0"}) MERGE (u)-[:TAGGED {tag: "preserve"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBESKYG0"}) MERGE (u)-[:TAGGED {tag: "satisfied"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBESKYG0"}) MERGE (u)-[:TAGGED {tag: "sentimental"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBESKYG0"}) MERGE (u)-[:TAGGED {tag: "yum"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESNE00"}) MERGE (u)-[:TAGGED {tag: "enthuse"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESNE00"}) MERGE (u)-[:TAGGED {tag: "including"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESNE00"}) MERGE (u)-[:TAGGED {tag: "knavishly"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESNE00"}) MERGE (u)-[:TAGGED {tag: "within"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBESPR00"}) MERGE (u)-[:TAGGED {tag: "drummer"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBESPR00"}) MERGE (u)-[:TAGGED {tag: "inasmuch"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBESPR00"}) MERGE (u)-[:TAGGED {tag: "than"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBESPR00"}) MERGE (u)-[:TAGGED {tag: "within"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBESPR00"}) MERGE (u)-[:TAGGED {tag: "yearningly"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBESRFG0"}) MERGE (u)-[:TAGGED {tag: "behind"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBESRFG0"}) MERGE (u)-[:TAGGED {tag: "cardigan"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBESRFG0"}) MERGE (u)-[:TAGGED {tag: "healthily"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBESRFG0"}) MERGE (u)-[:TAGGED {tag: "inasmuch"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBESRFG0"}) MERGE (u)-[:TAGGED {tag: "quirkily"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBESRFG0"}) MERGE (u)-[:TAGGED {tag: "wealthy"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBESRFG0"}) MERGE (u)-[:TAGGED {tag: "yum"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBEST8G0"}) MERGE (u)-[:TAGGED {tag: "bashfully"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBEST8G0"}) MERGE (u)-[:TAGGED {tag: "behind"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBEST8G0"}) MERGE (u)-[:TAGGED {tag: "but"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBEST8G0"}) MERGE (u)-[:TAGGED {tag: "cardigan"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBEST8G0"}) MERGE (u)-[:TAGGED {tag: "cardigan"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBEST8G0"}) MERGE (u)-[:TAGGED {tag: "knavishly"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBEST8G0"}) MERGE (u)-[:TAGGED {tag: "opportunity"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESVC00"}) MERGE (u)-[:TAGGED {tag: "behind"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESVC00"}) MERGE (u)-[:TAGGED {tag: "preserve"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBESVC00"}) MERGE (u)-[:TAGGED {tag: "yearningly"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBESWP00"}) MERGE (u)-[:TAGGED {tag: "within"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBESWP00"}) MERGE (u)-[:TAGGED {tag: "yearningly"}]->(p);
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBESY900"}) MERGE (u)-[:TAGGED {tag: "drummer"}]->(p);
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBESY900"}) MERGE (u)-[:TAGGED {tag: "how"}]->(p);
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBESY900"}) MERGE (u)-[:TAGGED {tag: "instantly"}]->(p);
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBESY900"}) MERGE (u)-[:TAGGED {tag: "sentimental"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBESZCG0"}) MERGE (u)-[:TAGGED {tag: "behind"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBESZCG0"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBESZCG0"}) MERGE (u)-[:TAGGED {tag: "rule"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET1P00"}) MERGE (u)-[:TAGGED {tag: "bashfully"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET1P00"}) MERGE (u)-[:TAGGED {tag: "flip"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET1P00"}) MERGE (u)-[:TAGGED {tag: "forecast"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET1P00"}) MERGE (u)-[:TAGGED {tag: "gosh"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET1P00"}) MERGE (u)-[:TAGGED {tag: "inevitable"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET1P00"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET1P00"}) MERGE (u)-[:TAGGED {tag: "satisfied"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET4BG0"}) MERGE (u)-[:TAGGED {tag: "anti"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET4BG0"}) MERGE (u)-[:TAGGED {tag: "healthily"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET4BG0"}) MERGE (u)-[:TAGGED {tag: "hungrily"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET4BG0"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET4BG0"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET4BG0"}) MERGE (u)-[:TAGGED {tag: "sentimental"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBET4BG0"}) MERGE (u)-[:TAGGED {tag: "yum"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBET5GG0"}) MERGE (u)-[:TAGGED {tag: "enthuse"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBET5GG0"}) MERGE (u)-[:TAGGED {tag: "flip"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBET5GG0"}) MERGE (u)-[:TAGGED {tag: "hungrily"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBET5GG0"}) MERGE (u)-[:TAGGED {tag: "psst"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBET5GG0"}) MERGE (u)-[:TAGGED {tag: "wealthy"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBET5GG0"}) MERGE (u)-[:TAGGED {tag: "yet"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBET6PG0"}) MERGE (u)-[:TAGGED {tag: "about"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBET6PG0"}) MERGE (u)-[:TAGGED {tag: "anti"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBET6PG0"}) MERGE (u)-[:TAGGED {tag: "deeply"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBET6PG0"}) MERGE (u)-[:TAGGED {tag: "mastication"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBET6PG0"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBET6PG0"}) MERGE (u)-[:TAGGED {tag: "sentimental"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBET6PG0"}) MERGE (u)-[:TAGGED {tag: "sentimental"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBET6PG0"}) MERGE (u)-[:TAGGED {tag: "yearningly"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET89G0"}) MERGE (u)-[:TAGGED {tag: "as"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET89G0"}) MERGE (u)-[:TAGGED {tag: "cardigan"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET89G0"}) MERGE (u)-[:TAGGED {tag: "establishment"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET89G0"}) MERGE (u)-[:TAGGED {tag: "forecast"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET89G0"}) MERGE (u)-[:TAGGED {tag: "how"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET89G0"}) MERGE (u)-[:TAGGED {tag: "innocent"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET89G0"}) MERGE (u)-[:TAGGED {tag: "sentimental"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET9MG0"}) MERGE (u)-[:TAGGED {tag: "catalogue"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET9MG0"}) MERGE (u)-[:TAGGED {tag: "enthuse"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET9MG0"}) MERGE (u)-[:TAGGED {tag: "flip"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET9MG0"}) MERGE (u)-[:TAGGED {tag: "inevitable"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET9MG0"}) MERGE (u)-[:TAGGED {tag: "manicure"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBET9MG0"}) MERGE (u)-[:TAGGED {tag: "rule"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBETAWG0"}) MERGE (u)-[:TAGGED {tag: "how"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBETAWG0"}) MERGE (u)-[:TAGGED {tag: "inasmuch"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBETAWG0"}) MERGE (u)-[:TAGGED {tag: "opportunity"}]->(p);
MATCH (u:User {id: "ajtxy96ycctqu3kwrm5pkf4udgkdda5qdsckqzsdj7nffcnpx3go"}), (p:Post {id: "2Z1N8QBETAWG0"}) MERGE (u)-[:TAGGED {tag: "partially"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBETC900"}) MERGE (u)-[:TAGGED {tag: "beside"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBETC900"}) MERGE (u)-[:TAGGED {tag: "denominator"}]->(p);
MATCH (u:User {id: "pnd8q4uyypjjbffpgxcfmxiu7fbss38fqsnt5xz8da13nj6q5obo"}), (p:Post {id: "2Z1N8QBETC900"}) MERGE (u)-[:TAGGED {tag: "than"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETDS00"}) MERGE (u)-[:TAGGED {tag: "buckle"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETDS00"}) MERGE (u)-[:TAGGED {tag: "buckle"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETDS00"}) MERGE (u)-[:TAGGED {tag: "how"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETDS00"}) MERGE (u)-[:TAGGED {tag: "including"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETDS00"}) MERGE (u)-[:TAGGED {tag: "less"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETDS00"}) MERGE (u)-[:TAGGED {tag: "manicure"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETDS00"}) MERGE (u)-[:TAGGED {tag: "offensively"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETDS00"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETDS00"}) MERGE (u)-[:TAGGED {tag: "sentimental"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETEYG0"}) MERGE (u)-[:TAGGED {tag: "buckle"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETEYG0"}) MERGE (u)-[:TAGGED {tag: "but"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETEYG0"}) MERGE (u)-[:TAGGED {tag: "how"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETEYG0"}) MERGE (u)-[:TAGGED {tag: "instantly"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETEYG0"}) MERGE (u)-[:TAGGED {tag: "legume"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETEYG0"}) MERGE (u)-[:TAGGED {tag: "satisfied"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETG5G0"}) MERGE (u)-[:TAGGED {tag: "as"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETG5G0"}) MERGE (u)-[:TAGGED {tag: "bashfully"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETG5G0"}) MERGE (u)-[:TAGGED {tag: "denominator"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETG5G0"}) MERGE (u)-[:TAGGED {tag: "inevitable"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETG5G0"}) MERGE (u)-[:TAGGED {tag: "innocent"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETG5G0"}) MERGE (u)-[:TAGGED {tag: "preserve"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETHK00"}) MERGE (u)-[:TAGGED {tag: "anti"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETHK00"}) MERGE (u)-[:TAGGED {tag: "as"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETHK00"}) MERGE (u)-[:TAGGED {tag: "enthuse"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETHK00"}) MERGE (u)-[:TAGGED {tag: "enthuse"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETHK00"}) MERGE (u)-[:TAGGED {tag: "establishment"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETHK00"}) MERGE (u)-[:TAGGED {tag: "healthily"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETHK00"}) MERGE (u)-[:TAGGED {tag: "innocent"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETHK00"}) MERGE (u)-[:TAGGED {tag: "partially"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETHK00"}) MERGE (u)-[:TAGGED {tag: "yet"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBETJMG0"}) MERGE (u)-[:TAGGED {tag: "cardigan"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBETJMG0"}) MERGE (u)-[:TAGGED {tag: "deeply"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBETJMG0"}) MERGE (u)-[:TAGGED {tag: "inevitable"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBETJMG0"}) MERGE (u)-[:TAGGED {tag: "legume"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBETJMG0"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBETJMG0"}) MERGE (u)-[:TAGGED {tag: "psst"}]->(p);
MATCH (u:User {id: "pxccbsm8ic355nmd3hufjq47s4h8snosbbxoo7jqn3otgs9azgco"}), (p:Post {id: "2Z1N8QBETJMG0"}) MERGE (u)-[:TAGGED {tag: "sentimental"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETKR00"}) MERGE (u)-[:TAGGED {tag: "as"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETKR00"}) MERGE (u)-[:TAGGED {tag: "but"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETKR00"}) MERGE (u)-[:TAGGED {tag: "drummer"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETKR00"}) MERGE (u)-[:TAGGED {tag: "manicure"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETKR00"}) MERGE (u)-[:TAGGED {tag: "satisfied"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETKR00"}) MERGE (u)-[:TAGGED {tag: "wealthy"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETN1G0"}) MERGE (u)-[:TAGGED {tag: "beside"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETN1G0"}) MERGE (u)-[:TAGGED {tag: "cardigan"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETN1G0"}) MERGE (u)-[:TAGGED {tag: "catalogue"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETN1G0"}) MERGE (u)-[:TAGGED {tag: "healthily"}]->(p);
MATCH (u:User {id: "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y"}), (p:Post {id: "2Z1N8QBETN1G0"}) MERGE (u)-[:TAGGED {tag: "mastication"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETP6G0"}) MERGE (u)-[:TAGGED {tag: "bashfully"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETP6G0"}) MERGE (u)-[:TAGGED {tag: "cardigan"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETP6G0"}) MERGE (u)-[:TAGGED {tag: "deeply"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETP6G0"}) MERGE (u)-[:TAGGED {tag: "how"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETP6G0"}) MERGE (u)-[:TAGGED {tag: "inevitable"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETP6G0"}) MERGE (u)-[:TAGGED {tag: "quirkily"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETP6G0"}) MERGE (u)-[:TAGGED {tag: "rule"}]->(p);
MATCH (u:User {id: "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so"}), (p:Post {id: "2Z1N8QBETP6G0"}) MERGE (u)-[:TAGGED {tag: "yearningly"}]->(p);
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBETQK00"}) MERGE (u)-[:TAGGED {tag: "bashfully"}]->(p);
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBETQK00"}) MERGE (u)-[:TAGGED {tag: "catalogue"}]->(p);
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBETQK00"}) MERGE (u)-[:TAGGED {tag: "how"}]->(p);
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBETQK00"}) MERGE (u)-[:TAGGED {tag: "less"}]->(p);
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBETQK00"}) MERGE (u)-[:TAGGED {tag: "manicure"}]->(p);
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBETQK00"}) MERGE (u)-[:TAGGED {tag: "manicure"}]->(p);
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBETQK00"}) MERGE (u)-[:TAGGED {tag: "once"}]->(p);
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBETQK00"}) MERGE (u)-[:TAGGED {tag: "test"}]->(p);
MATCH (u:User {id: "j6fcwdbf4owi6njn9zrm7tu3tjw3a5itdt9i1rezir7zemtaagyy"}), (p:Post {id: "2Z1N8QBETQK00"}) MERGE (u)-[:TAGGED {tag: "wealthy"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETRYG0"}) MERGE (u)-[:TAGGED {tag: "catalogue"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETRYG0"}) MERGE (u)-[:TAGGED {tag: "inasmuch"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETRYG0"}) MERGE (u)-[:TAGGED {tag: "quirkily"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETTM00"}) MERGE (u)-[:TAGGED {tag: "about"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETTM00"}) MERGE (u)-[:TAGGED {tag: "about"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETTM00"}) MERGE (u)-[:TAGGED {tag: "deeply"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETTM00"}) MERGE (u)-[:TAGGED {tag: "drummer"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETTM00"}) MERGE (u)-[:TAGGED {tag: "enthuse"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETTM00"}) MERGE (u)-[:TAGGED {tag: "hungrily"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETTM00"}) MERGE (u)-[:TAGGED {tag: "less"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETTM00"}) MERGE (u)-[:TAGGED {tag: "preserve"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETTM00"}) MERGE (u)-[:TAGGED {tag: "sentimental"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETW700"}) MERGE (u)-[:TAGGED {tag: "but"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETW700"}) MERGE (u)-[:TAGGED {tag: "drummer"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETW700"}) MERGE (u)-[:TAGGED {tag: "less"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETW700"}) MERGE (u)-[:TAGGED {tag: "offensively"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETW700"}) MERGE (u)-[:TAGGED {tag: "offensively"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETW700"}) MERGE (u)-[:TAGGED {tag: "satisfied"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETW700"}) MERGE (u)-[:TAGGED {tag: "sentimental"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETW700"}) MERGE (u)-[:TAGGED {tag: "under"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETW700"}) MERGE (u)-[:TAGGED {tag: "under"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBETW700"}) MERGE (u)-[:TAGGED {tag: "under"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETXT00"}) MERGE (u)-[:TAGGED {tag: "denominator"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETXT00"}) MERGE (u)-[:TAGGED {tag: "inasmuch"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETXT00"}) MERGE (u)-[:TAGGED {tag: "mastication"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETXT00"}) MERGE (u)-[:TAGGED {tag: "preserve"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETXT00"}) MERGE (u)-[:TAGGED {tag: "yearningly"}]->(p);
MATCH (u:User {id: "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y"}), (p:Post {id: "2Z1N8QBETXT00"}) MERGE (u)-[:TAGGED {tag: "yet"}]->(p);
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBETZ1G0"}) MERGE (u)-[:TAGGED {tag: "drummer"}]->(p);
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBETZ1G0"}) MERGE (u)-[:TAGGED {tag: "drummer"}]->(p);
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBETZ1G0"}) MERGE (u)-[:TAGGED {tag: "instantly"}]->(p);
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBETZ1G0"}) MERGE (u)-[:TAGGED {tag: "preserve"}]->(p);
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBETZ1G0"}) MERGE (u)-[:TAGGED {tag: "preserve"}]->(p);
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBETZ1G0"}) MERGE (u)-[:TAGGED {tag: "quirkily"}]->(p);
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBETZ1G0"}) MERGE (u)-[:TAGGED {tag: "rule"}]->(p);
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBEV03G0"}) MERGE (u)-[:TAGGED {tag: "opportunity"}]->(p);
MATCH (u:User {id: "uzisj5op5ynk83m9j1fnbfqnx44jbj4h9rjtzwbpt57nts19z1uy"}), (p:Post {id: "2Z1N8QBEV03G0"}) MERGE (u)-[:TAGGED {tag: "rule"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBEV1D00"}) MERGE (u)-[:TAGGED {tag: "establishment"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBEV1D00"}) MERGE (u)-[:TAGGED {tag: "inevitable"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBEV1D00"}) MERGE (u)-[:TAGGED {tag: "manicure"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBEV1D00"}) MERGE (u)-[:TAGGED {tag: "quirkily"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBEV1D00"}) MERGE (u)-[:TAGGED {tag: "sentimental"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBEV1D00"}) MERGE (u)-[:TAGGED {tag: "wealthy"}]->(p);
MATCH (u:User {id: "kujmtrs7spa53btsufix7phj6frc1eymeqbctbu5s9fgxcsnjy6o"}), (p:Post {id: "2Z1N8QBEV1D00"}) MERGE (u)-[:TAGGED {tag: "yet"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56VY8G0"}) MERGE (u)-[:TAGGED {tag: "cute"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56VY8G0"}) MERGE (u)-[:TAGGED {tag: "irritably"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56VY8G0"}) MERGE (u)-[:TAGGED {tag: "writ"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W1EG0"}) MERGE (u)-[:TAGGED {tag: "cute"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W1EG0"}) MERGE (u)-[:TAGGED {tag: "dapper"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W1EG0"}) MERGE (u)-[:TAGGED {tag: "even"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W1EG0"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W1EG0"}) MERGE (u)-[:TAGGED {tag: "hot"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W1EG0"}) MERGE (u)-[:TAGGED {tag: "if"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W1EG0"}) MERGE (u)-[:TAGGED {tag: "potential"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W1EG0"}) MERGE (u)-[:TAGGED {tag: "towards"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W2DG0"}) MERGE (u)-[:TAGGED {tag: "cheap"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W2DG0"}) MERGE (u)-[:TAGGED {tag: "emergent"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W2DG0"}) MERGE (u)-[:TAGGED {tag: "frantically"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W2DG0"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W2DG0"}) MERGE (u)-[:TAGGED {tag: "irritably"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W2DG0"}) MERGE (u)-[:TAGGED {tag: "since"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W2DG0"}) MERGE (u)-[:TAGGED {tag: "towards"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W2DG0"}) MERGE (u)-[:TAGGED {tag: "wildly"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56W3CG0"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56W3CG0"}) MERGE (u)-[:TAGGED {tag: "recklessly"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56W3ZG0"}) MERGE (u)-[:TAGGED {tag: "amid"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56W3ZG0"}) MERGE (u)-[:TAGGED {tag: "excuse"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56W3ZG0"}) MERGE (u)-[:TAGGED {tag: "frantically"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56W3ZG0"}) MERGE (u)-[:TAGGED {tag: "hot"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56W3ZG0"}) MERGE (u)-[:TAGGED {tag: "knowing"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56W3ZG0"}) MERGE (u)-[:TAGGED {tag: "nor"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56W3ZG0"}) MERGE (u)-[:TAGGED {tag: "sadly"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56W3ZG0"}) MERGE (u)-[:TAGGED {tag: "wildly"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W4VG0"}) MERGE (u)-[:TAGGED {tag: "err"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W4VG0"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W4VG0"}) MERGE (u)-[:TAGGED {tag: "hot"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W4VG0"}) MERGE (u)-[:TAGGED {tag: "if"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W4VG0"}) MERGE (u)-[:TAGGED {tag: "variable"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W4VG0"}) MERGE (u)-[:TAGGED {tag: "watchful"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W4VG0"}) MERGE (u)-[:TAGGED {tag: "webbed"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56W5N00"}) MERGE (u)-[:TAGGED {tag: "and"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56W5N00"}) MERGE (u)-[:TAGGED {tag: "dapper"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56W5N00"}) MERGE (u)-[:TAGGED {tag: "since"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56W5N00"}) MERGE (u)-[:TAGGED {tag: "variable"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56W6MG0"}) MERGE (u)-[:TAGGED {tag: "after"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56W6MG0"}) MERGE (u)-[:TAGGED {tag: "emergent"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56W6MG0"}) MERGE (u)-[:TAGGED {tag: "emergent"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56W6MG0"}) MERGE (u)-[:TAGGED {tag: "expiate"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56W6MG0"}) MERGE (u)-[:TAGGED {tag: "knowing"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56W6MG0"}) MERGE (u)-[:TAGGED {tag: "pish"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56W6MG0"}) MERGE (u)-[:TAGGED {tag: "webbed"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W7M00"}) MERGE (u)-[:TAGGED {tag: "err"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W7M00"}) MERGE (u)-[:TAGGED {tag: "nor"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W7M00"}) MERGE (u)-[:TAGGED {tag: "nor"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W7M00"}) MERGE (u)-[:TAGGED {tag: "ugh"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W7M00"}) MERGE (u)-[:TAGGED {tag: "variable"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56W7M00"}) MERGE (u)-[:TAGGED {tag: "yaw"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W8D00"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W8D00"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W8D00"}) MERGE (u)-[:TAGGED {tag: "hmph"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W8D00"}) MERGE (u)-[:TAGGED {tag: "hot"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W8D00"}) MERGE (u)-[:TAGGED {tag: "knowing"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W8D00"}) MERGE (u)-[:TAGGED {tag: "mmm"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W8D00"}) MERGE (u)-[:TAGGED {tag: "mmm"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W8D00"}) MERGE (u)-[:TAGGED {tag: "towards"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56W8D00"}) MERGE (u)-[:TAGGED {tag: "webbed"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56W9K00"}) MERGE (u)-[:TAGGED {tag: "aw"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56W9K00"}) MERGE (u)-[:TAGGED {tag: "cute"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56W9K00"}) MERGE (u)-[:TAGGED {tag: "emergent"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56W9K00"}) MERGE (u)-[:TAGGED {tag: "excuse"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56W9K00"}) MERGE (u)-[:TAGGED {tag: "irritably"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56W9K00"}) MERGE (u)-[:TAGGED {tag: "potential"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56W9K00"}) MERGE (u)-[:TAGGED {tag: "towards"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WA600"}) MERGE (u)-[:TAGGED {tag: "excuse"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WA600"}) MERGE (u)-[:TAGGED {tag: "hmph"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WA600"}) MERGE (u)-[:TAGGED {tag: "madly"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WA600"}) MERGE (u)-[:TAGGED {tag: "ugh"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WG1G0"}) MERGE (u)-[:TAGGED {tag: "after"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WG1G0"}) MERGE (u)-[:TAGGED {tag: "amid"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WG1G0"}) MERGE (u)-[:TAGGED {tag: "nor"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WG1G0"}) MERGE (u)-[:TAGGED {tag: "nor"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WG1G0"}) MERGE (u)-[:TAGGED {tag: "recklessly"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WG1G0"}) MERGE (u)-[:TAGGED {tag: "variable"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WG1G0"}) MERGE (u)-[:TAGGED {tag: "wicked"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WG1G0"}) MERGE (u)-[:TAGGED {tag: "writ"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56WGYG0"}) MERGE (u)-[:TAGGED {tag: "even"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56WGYG0"}) MERGE (u)-[:TAGGED {tag: "watchful"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WHQG0"}) MERGE (u)-[:TAGGED {tag: "after"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WHQG0"}) MERGE (u)-[:TAGGED {tag: "computerise"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WHQG0"}) MERGE (u)-[:TAGGED {tag: "computerise"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WHQG0"}) MERGE (u)-[:TAGGED {tag: "duel"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WHQG0"}) MERGE (u)-[:TAGGED {tag: "err"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WHQG0"}) MERGE (u)-[:TAGGED {tag: "heavily"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WHQG0"}) MERGE (u)-[:TAGGED {tag: "recklessly"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WHQG0"}) MERGE (u)-[:TAGGED {tag: "wildly"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WJKG0"}) MERGE (u)-[:TAGGED {tag: "after"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WJKG0"}) MERGE (u)-[:TAGGED {tag: "err"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WJKG0"}) MERGE (u)-[:TAGGED {tag: "frantically"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WJKG0"}) MERGE (u)-[:TAGGED {tag: "likewise"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WJKG0"}) MERGE (u)-[:TAGGED {tag: "mmm"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WJKG0"}) MERGE (u)-[:TAGGED {tag: "since"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WJKG0"}) MERGE (u)-[:TAGGED {tag: "ugh"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WJKG0"}) MERGE (u)-[:TAGGED {tag: "wisely"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WJKG0"}) MERGE (u)-[:TAGGED {tag: "yaw"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WKE00"}) MERGE (u)-[:TAGGED {tag: "computerise"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WKE00"}) MERGE (u)-[:TAGGED {tag: "emergent"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WKE00"}) MERGE (u)-[:TAGGED {tag: "nor"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WKE00"}) MERGE (u)-[:TAGGED {tag: "variable"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WKE00"}) MERGE (u)-[:TAGGED {tag: "watchful"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WKE00"}) MERGE (u)-[:TAGGED {tag: "writ"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WN8G0"}) MERGE (u)-[:TAGGED {tag: "and"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WN8G0"}) MERGE (u)-[:TAGGED {tag: "duel"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WN8G0"}) MERGE (u)-[:TAGGED {tag: "potential"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WN8G0"}) MERGE (u)-[:TAGGED {tag: "since"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WN8G0"}) MERGE (u)-[:TAGGED {tag: "since"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WN8G0"}) MERGE (u)-[:TAGGED {tag: "ugh"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WN8G0"}) MERGE (u)-[:TAGGED {tag: "wicked"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WN8G0"}) MERGE (u)-[:TAGGED {tag: "writ"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WN8G0"}) MERGE (u)-[:TAGGED {tag: "yaw"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WNX00"}) MERGE (u)-[:TAGGED {tag: "cheap"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WNX00"}) MERGE (u)-[:TAGGED {tag: "knowing"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WNX00"}) MERGE (u)-[:TAGGED {tag: "mmm"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WNX00"}) MERGE (u)-[:TAGGED {tag: "ugh"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WPS00"}) MERGE (u)-[:TAGGED {tag: "cute"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WPS00"}) MERGE (u)-[:TAGGED {tag: "hot"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WPS00"}) MERGE (u)-[:TAGGED {tag: "variable"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56WQFG0"}) MERGE (u)-[:TAGGED {tag: "emergent"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56WQFG0"}) MERGE (u)-[:TAGGED {tag: "err"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56WQFG0"}) MERGE (u)-[:TAGGED {tag: "even"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56WQFG0"}) MERGE (u)-[:TAGGED {tag: "excuse"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56WQFG0"}) MERGE (u)-[:TAGGED {tag: "if"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56WQFG0"}) MERGE (u)-[:TAGGED {tag: "if"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56WQFG0"}) MERGE (u)-[:TAGGED {tag: "likewise"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56WQFG0"}) MERGE (u)-[:TAGGED {tag: "watchful"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WREG0"}) MERGE (u)-[:TAGGED {tag: "amid"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WREG0"}) MERGE (u)-[:TAGGED {tag: "aw"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WREG0"}) MERGE (u)-[:TAGGED {tag: "err"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WREG0"}) MERGE (u)-[:TAGGED {tag: "nor"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WREG0"}) MERGE (u)-[:TAGGED {tag: "oh"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56WREG0"}) MERGE (u)-[:TAGGED {tag: "watchful"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WSCG0"}) MERGE (u)-[:TAGGED {tag: "even"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WSCG0"}) MERGE (u)-[:TAGGED {tag: "expiate"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WSCG0"}) MERGE (u)-[:TAGGED {tag: "if"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WSCG0"}) MERGE (u)-[:TAGGED {tag: "likewise"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WSCG0"}) MERGE (u)-[:TAGGED {tag: "nor"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WSCG0"}) MERGE (u)-[:TAGGED {tag: "sadly"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WSCG0"}) MERGE (u)-[:TAGGED {tag: "since"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WSCG0"}) MERGE (u)-[:TAGGED {tag: "webbed"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WSCG0"}) MERGE (u)-[:TAGGED {tag: "wicked"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56WT600"}) MERGE (u)-[:TAGGED {tag: "likewise"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56WTQG0"}) MERGE (u)-[:TAGGED {tag: "cute"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56WTQG0"}) MERGE (u)-[:TAGGED {tag: "excuse"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WVEG0"}) MERGE (u)-[:TAGGED {tag: "after"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WVEG0"}) MERGE (u)-[:TAGGED {tag: "even"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WVEG0"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WVEG0"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WVEG0"}) MERGE (u)-[:TAGGED {tag: "irritably"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WVEG0"}) MERGE (u)-[:TAGGED {tag: "knowing"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WVEG0"}) MERGE (u)-[:TAGGED {tag: "nervously"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WVEG0"}) MERGE (u)-[:TAGGED {tag: "nor"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WVEG0"}) MERGE (u)-[:TAGGED {tag: "ugh"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WW600"}) MERGE (u)-[:TAGGED {tag: "emergent"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WW600"}) MERGE (u)-[:TAGGED {tag: "err"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WW600"}) MERGE (u)-[:TAGGED {tag: "madly"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WW600"}) MERGE (u)-[:TAGGED {tag: "sadly"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56WW600"}) MERGE (u)-[:TAGGED {tag: "yaw"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56WX200"}) MERGE (u)-[:TAGGED {tag: "amid"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56WX200"}) MERGE (u)-[:TAGGED {tag: "hmph"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56WX200"}) MERGE (u)-[:TAGGED {tag: "hot"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56WX200"}) MERGE (u)-[:TAGGED {tag: "hot"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56WX200"}) MERGE (u)-[:TAGGED {tag: "irritably"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56WX200"}) MERGE (u)-[:TAGGED {tag: "mutation"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56WX200"}) MERGE (u)-[:TAGGED {tag: "mutation"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56WX200"}) MERGE (u)-[:TAGGED {tag: "ugh"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56WXJ00"}) MERGE (u)-[:TAGGED {tag: "after"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56WXJ00"}) MERGE (u)-[:TAGGED {tag: "dapper"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56WXJ00"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56WXJ00"}) MERGE (u)-[:TAGGED {tag: "irritably"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WY1G0"}) MERGE (u)-[:TAGGED {tag: "irritably"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WY1G0"}) MERGE (u)-[:TAGGED {tag: "sadly"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WY1G0"}) MERGE (u)-[:TAGGED {tag: "sadly"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WY1G0"}) MERGE (u)-[:TAGGED {tag: "variable"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56WY1G0"}) MERGE (u)-[:TAGGED {tag: "watchful"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WYXG0"}) MERGE (u)-[:TAGGED {tag: "cute"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WYXG0"}) MERGE (u)-[:TAGGED {tag: "excuse"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WYXG0"}) MERGE (u)-[:TAGGED {tag: "hot"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WYXG0"}) MERGE (u)-[:TAGGED {tag: "nor"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WYXG0"}) MERGE (u)-[:TAGGED {tag: "potential"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56WYXG0"}) MERGE (u)-[:TAGGED {tag: "webbed"}]->(p);
MATCH (u:User {id: "xgoium16biftc8z7ecrbwrdokdwr1xphdke4nuaqiutnihdmxmny"}), (p:Post {id: "2Z1N9M56WZK00"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "xgoium16biftc8z7ecrbwrdokdwr1xphdke4nuaqiutnihdmxmny"}), (p:Post {id: "2Z1N9M56WZK00"}) MERGE (u)-[:TAGGED {tag: "ugh"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X0D00"}) MERGE (u)-[:TAGGED {tag: "and"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X0D00"}) MERGE (u)-[:TAGGED {tag: "dapper"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X0D00"}) MERGE (u)-[:TAGGED {tag: "dapper"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X0D00"}) MERGE (u)-[:TAGGED {tag: "emergent"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X0D00"}) MERGE (u)-[:TAGGED {tag: "expiate"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X0D00"}) MERGE (u)-[:TAGGED {tag: "mmm"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X0D00"}) MERGE (u)-[:TAGGED {tag: "since"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X0XG0"}) MERGE (u)-[:TAGGED {tag: "after"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X0XG0"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X0XG0"}) MERGE (u)-[:TAGGED {tag: "if"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X0XG0"}) MERGE (u)-[:TAGGED {tag: "wisely"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X1SG0"}) MERGE (u)-[:TAGGED {tag: "excuse"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X1SG0"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X1SG0"}) MERGE (u)-[:TAGGED {tag: "hot"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X1SG0"}) MERGE (u)-[:TAGGED {tag: "if"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X1SG0"}) MERGE (u)-[:TAGGED {tag: "towards"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X2C00"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X2C00"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X2C00"}) MERGE (u)-[:TAGGED {tag: "hot"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X2C00"}) MERGE (u)-[:TAGGED {tag: "mmm"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X2C00"}) MERGE (u)-[:TAGGED {tag: "watchful"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X2C00"}) MERGE (u)-[:TAGGED {tag: "wisely"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56X2ZG0"}) MERGE (u)-[:TAGGED {tag: "computerise"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56X2ZG0"}) MERGE (u)-[:TAGGED {tag: "cute"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56X2ZG0"}) MERGE (u)-[:TAGGED {tag: "emergent"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56X2ZG0"}) MERGE (u)-[:TAGGED {tag: "heavily"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56X2ZG0"}) MERGE (u)-[:TAGGED {tag: "lest"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56X2ZG0"}) MERGE (u)-[:TAGGED {tag: "variable"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56X3N00"}) MERGE (u)-[:TAGGED {tag: "aw"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56X3N00"}) MERGE (u)-[:TAGGED {tag: "aw"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56X3N00"}) MERGE (u)-[:TAGGED {tag: "if"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56X3N00"}) MERGE (u)-[:TAGGED {tag: "pish"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56X3N00"}) MERGE (u)-[:TAGGED {tag: "watchful"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X4EG0"}) MERGE (u)-[:TAGGED {tag: "cheap"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X4EG0"}) MERGE (u)-[:TAGGED {tag: "duel"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X4EG0"}) MERGE (u)-[:TAGGED {tag: "emergent"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X4EG0"}) MERGE (u)-[:TAGGED {tag: "frantically"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X4EG0"}) MERGE (u)-[:TAGGED {tag: "irritably"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X4EG0"}) MERGE (u)-[:TAGGED {tag: "mutation"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X4EG0"}) MERGE (u)-[:TAGGED {tag: "nor"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X4EG0"}) MERGE (u)-[:TAGGED {tag: "pish"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X4EG0"}) MERGE (u)-[:TAGGED {tag: "ugh"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56X4EG0"}) MERGE (u)-[:TAGGED {tag: "wildly"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56X55G0"}) MERGE (u)-[:TAGGED {tag: "cheap"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56X55G0"}) MERGE (u)-[:TAGGED {tag: "err"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56X55G0"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "ghi8imi1o3i4c18q3anyp3qj8m9cedrrif1tco8jouq8msud8mfy"}), (p:Post {id: "2Z1N9M56X55G0"}) MERGE (u)-[:TAGGED {tag: "potential"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X5VG0"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X5VG0"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X5VG0"}) MERGE (u)-[:TAGGED {tag: "mmm"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X5VG0"}) MERGE (u)-[:TAGGED {tag: "nor"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X5VG0"}) MERGE (u)-[:TAGGED {tag: "oh"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X5VG0"}) MERGE (u)-[:TAGGED {tag: "pish"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X5VG0"}) MERGE (u)-[:TAGGED {tag: "sadly"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X5VG0"}) MERGE (u)-[:TAGGED {tag: "since"}]->(p);
MATCH (u:User {id: "xzinu5xbj55y33kfxe9sgtu9a4gxqgywj68oi91g6apiym8kz6to"}), (p:Post {id: "2Z1N9M56X5VG0"}) MERGE (u)-[:TAGGED {tag: "yaw"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X6JG0"}) MERGE (u)-[:TAGGED {tag: "and"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X6JG0"}) MERGE (u)-[:TAGGED {tag: "frantically"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X6JG0"}) MERGE (u)-[:TAGGED {tag: "nor"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X6JG0"}) MERGE (u)-[:TAGGED {tag: "pish"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X6JG0"}) MERGE (u)-[:TAGGED {tag: "recklessly"}]->(p);
MATCH (u:User {id: "me9hsyatet99rgxcabaedoizr646x9i9k5doh4zcjfdb3u9s6hgy"}), (p:Post {id: "2Z1N9M56X6JG0"}) MERGE (u)-[:TAGGED {tag: "since"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X7DG0"}) MERGE (u)-[:TAGGED {tag: "after"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X7DG0"}) MERGE (u)-[:TAGGED {tag: "and"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X7DG0"}) MERGE (u)-[:TAGGED {tag: "hot"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X7DG0"}) MERGE (u)-[:TAGGED {tag: "if"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X7DG0"}) MERGE (u)-[:TAGGED {tag: "if"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X7DG0"}) MERGE (u)-[:TAGGED {tag: "lest"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X7DG0"}) MERGE (u)-[:TAGGED {tag: "likewise"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X7DG0"}) MERGE (u)-[:TAGGED {tag: "nor"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56X7DG0"}) MERGE (u)-[:TAGGED {tag: "wisely"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56X7Y00"}) MERGE (u)-[:TAGGED {tag: "err"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56X7Y00"}) MERGE (u)-[:TAGGED {tag: "mutation"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56X7Y00"}) MERGE (u)-[:TAGGED {tag: "wicked"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56X7Y00"}) MERGE (u)-[:TAGGED {tag: "wildly"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56X7Y00"}) MERGE (u)-[:TAGGED {tag: "writ"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56X8C00"}) MERGE (u)-[:TAGGED {tag: "after"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56X8C00"}) MERGE (u)-[:TAGGED {tag: "and"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56X8C00"}) MERGE (u)-[:TAGGED {tag: "and"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56X8C00"}) MERGE (u)-[:TAGGED {tag: "dapper"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56X8C00"}) MERGE (u)-[:TAGGED {tag: "since"}]->(p);
MATCH (u:User {id: "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy"}), (p:Post {id: "2Z1N9M56X8C00"}) MERGE (u)-[:TAGGED {tag: "since"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56X9000"}) MERGE (u)-[:TAGGED {tag: "duel"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56X9000"}) MERGE (u)-[:TAGGED {tag: "nervously"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56X9000"}) MERGE (u)-[:TAGGED {tag: "pish"}]->(p);
MATCH (u:User {id: "9eo5d4symyc9d5qshbznqiz5qc4a375f3oid6ixieqezxmb6ti5o"}), (p:Post {id: "2Z1N9M56X9000"}) MERGE (u)-[:TAGGED {tag: "since"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56X9M00"}) MERGE (u)-[:TAGGED {tag: "frantically"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56X9M00"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56X9M00"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56X9M00"}) MERGE (u)-[:TAGGED {tag: "hot"}]->(p);
MATCH (u:User {id: "zs9siw6qwazjk5beh6pu95nonzq1prp5oe3rbb916u67aiq5hezy"}), (p:Post {id: "2Z1N9M56X9M00"}) MERGE (u)-[:TAGGED {tag: "pish"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56XA1G0"}) MERGE (u)-[:TAGGED {tag: "aw"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56XA1G0"}) MERGE (u)-[:TAGGED {tag: "dapper"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56XA1G0"}) MERGE (u)-[:TAGGED {tag: "irritably"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56XA1G0"}) MERGE (u)-[:TAGGED {tag: "likewise"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56XA1G0"}) MERGE (u)-[:TAGGED {tag: "nervously"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56XA1G0"}) MERGE (u)-[:TAGGED {tag: "sadly"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56XA1G0"}) MERGE (u)-[:TAGGED {tag: "watchful"}]->(p);
MATCH (u:User {id: "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy"}), (p:Post {id: "2Z1N9M56XA1G0"}) MERGE (u)-[:TAGGED {tag: "wicked"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56XB2G0"}) MERGE (u)-[:TAGGED {tag: "cheap"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56XB2G0"}) MERGE (u)-[:TAGGED {tag: "computerise"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56XB2G0"}) MERGE (u)-[:TAGGED {tag: "even"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56XB2G0"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56XB2G0"}) MERGE (u)-[:TAGGED {tag: "towards"}]->(p);
MATCH (u:User {id: "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso"}), (p:Post {id: "2Z1N9M56XB2G0"}) MERGE (u)-[:TAGGED {tag: "yaw"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56XBP00"}) MERGE (u)-[:TAGGED {tag: "aw"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56XBP00"}) MERGE (u)-[:TAGGED {tag: "aw"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56XBP00"}) MERGE (u)-[:TAGGED {tag: "ha"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56XBP00"}) MERGE (u)-[:TAGGED {tag: "lipsum"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56XBP00"}) MERGE (u)-[:TAGGED {tag: "nervously"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56XBP00"}) MERGE (u)-[:TAGGED {tag: "pretty"}]->(p);
MATCH (u:User {id: "bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo"}), (p:Post {id: "2Z1N9M56XBP00"}) MERGE (u)-[:TAGGED {tag: "sadly"}]->(p);
MATCH (u:User {id: "ze86rtgp6x1qdyno4uzp8gexbb887dtemmonoh4j3iisbzitcppo"}), (p:Post {id: "2Z1NB44D42MG0"}) MERGE (u)-[:TAGGED {tag: "aw"}]->(p);
MATCH (u:User {id: "ze86rtgp6x1qdyno4uzp8gexbb887dtemmonoh4j3iisbzitcppo"}), (p:Post {id: "2Z1NB44D42MG0"}) MERGE (u)-[:TAGGED {tag: "aw"}]->(p);
MATCH (u:User {id: "ze86rtgp6x1qdyno4uzp8gexbb887dtemmonoh4j3iisbzitcppo"}), (p:Post {id: "2Z1NB44D42MG0"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}), (p:Post {id: "2Z1NB53BF82G0"}) MERGE (u)-[:TAGGED {tag: "noise"}]->(p);
MATCH (u:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}), (p:Post {id: "2Z1NB53BF82G0"}) MERGE (u)-[:TAGGED {tag: "noise"}]->(p);
MATCH (u:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}), (p:Post {id: "2Z1NB53BF82G0"}) MERGE (u)-[:TAGGED {tag: "noise"}]->(p);
MATCH (u:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}), (p:Post {id: "2Z1NB53BF82G0"}) MERGE (u)-[:TAGGED {tag: "noise"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NCPSDTW400"}) MERGE (u)-[:TAGGED {tag: "based"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NCPSDTW400"}) MERGE (u)-[:TAGGED {tag: "based"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NCPSDTW400"}) MERGE (u)-[:TAGGED {tag: "delulu"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NCPSDTW400"}) MERGE (u)-[:TAGGED {tag: "delulu"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NCPSDTW400"}) MERGE (u)-[:TAGGED {tag: "delulu"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NCPSDTW400"}) MERGE (u)-[:TAGGED {tag: "delulu"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NCPSDTW400"}) MERGE (u)-[:TAGGED {tag: "delulu"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NCPSDTW400"}) MERGE (u)-[:TAGGED {tag: "delulu"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NCPSDTW400"}) MERGE (u)-[:TAGGED {tag: "delulu"}]->(p);
MATCH (u:User {id: "end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso"}), (p:Post {id: "2Z1NCTJJXZTG0"}) MERGE (u)-[:TAGGED {tag: "Welcome"}]->(p);
MATCH (u:User {id: "end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso"}), (p:Post {id: "2Z1NCTJJXZTG0"}) MERGE (u)-[:TAGGED {tag: "Welcome"}]->(p);
MATCH (u:User {id: "end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso"}), (p:Post {id: "2Z1NCTJJXZTG0"}) MERGE (u)-[:TAGGED {tag: "Welcome"}]->(p);
MATCH (u:User {id: "end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso"}), (p:Post {id: "2Z1NCTJJXZTG0"}) MERGE (u)-[:TAGGED {tag: "aw"}]->(p);
MATCH (u:User {id: "end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso"}), (p:Post {id: "2Z1NCTJJXZTG0"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso"}), (p:Post {id: "2Z1NCTJJXZTG0"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}), (p:Post {id: "2Z1NDENYVN5G0"}) MERGE (u)-[:TAGGED {tag: "fightclub"}]->(p);
MATCH (u:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}), (p:Post {id: "2Z1NDENYVN5G0"}) MERGE (u)-[:TAGGED {tag: "fightclub"}]->(p);
MATCH (u:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}), (p:Post {id: "2Z1NDENYVN5G0"}) MERGE (u)-[:TAGGED {tag: "fightclub"}]->(p);
MATCH (u:User {id: "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno"}), (p:Post {id: "2Z1NDENYVN5G0"}) MERGE (u)-[:TAGGED {tag: "fightclub"}]->(p);
MATCH (u:User {id: "614ohi1318w3hts3bw89x3t4y8ctychdx6xm68prm478xrir1k8o"}), (p:Post {id: "2Z1NDJYBBBEG0"}) MERGE (u)-[:TAGGED {tag: "GM"}]->(p);
MATCH (u:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}), (p:Post {id: "2Z1NDNKKXYV00"}) MERGE (u)-[:TAGGED {tag: "who_is_this"}]->(p);
MATCH (u:User {id: "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco"}), (p:Post {id: "2Z1NGNZNVCM00"}) MERGE (u)-[:TAGGED {tag: "gm"}]->(p);
MATCH (u:User {id: "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco"}), (p:Post {id: "2Z1NGNZNVCM00"}) MERGE (u)-[:TAGGED {tag: "gm"}]->(p);
MATCH (u:User {id: "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco"}), (p:Post {id: "2Z1NGNZNVCM00"}) MERGE (u)-[:TAGGED {tag: "gm"}]->(p);
MATCH (u:User {id: "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco"}), (p:Post {id: "2Z1NGNZNVCM00"}) MERGE (u)-[:TAGGED {tag: "gm"}]->(p);
MATCH (u:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}), (p:Post {id: "2Z1NJ21ZTWW00"}) MERGE (u)-[:TAGGED {tag: "🖖"}]->(p);
MATCH (u:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}), (p:Post {id: "2Z1NJ21ZTWW00"}) MERGE (u)-[:TAGGED {tag: "🖖"}]->(p);
MATCH (u:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}), (p:Post {id: "2Z1NJ21ZTWW00"}) MERGE (u)-[:TAGGED {tag: "🖖"}]->(p);
MATCH (u:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}), (p:Post {id: "2Z1NJ21ZTWW00"}) MERGE (u)-[:TAGGED {tag: "🖖"}]->(p);
MATCH (u:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}), (p:Post {id: "2Z1NJ21ZTWW00"}) MERGE (u)-[:TAGGED {tag: "🖖"}]->(p);
MATCH (u:User {id: "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro"}), (p:Post {id: "2Z1NJ21ZTWW00"}) MERGE (u)-[:TAGGED {tag: "🖖"}]->(p);
MATCH (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (p:Post {id: "2Z1NJPW2QHGG0"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (p:Post {id: "2Z1NJPW2QHGG0"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (p:Post {id: "2Z1NJPW2QHGG0"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (p:Post {id: "2Z1NJPW2QHGG0"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (p:Post {id: "2Z1NJPW2QHGG0"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (p:Post {id: "2Z1NJPW2QHGG0"}) MERGE (u)-[:TAGGED {tag: "test"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1NP1EQ2PA00"}) MERGE (u)-[:TAGGED {tag: "protocol"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1NP1EQ2PA00"}) MERGE (u)-[:TAGGED {tag: "protocol"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1NP1EQ2PA00"}) MERGE (u)-[:TAGGED {tag: "protocol"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1NP1EQ2PA00"}) MERGE (u)-[:TAGGED {tag: "protocol"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1NP1EQ2PA00"}) MERGE (u)-[:TAGGED {tag: "protocol"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NRDN245400"}) MERGE (u)-[:TAGGED {tag: "I want"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NRDN245400"}) MERGE (u)-[:TAGGED {tag: "I want"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NRDN245400"}) MERGE (u)-[:TAGGED {tag: "I want"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NRDN245400"}) MERGE (u)-[:TAGGED {tag: "I want"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NRDN245400"}) MERGE (u)-[:TAGGED {tag: "I want"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1NRDN245400"}) MERGE (u)-[:TAGGED {tag: "Nooo"}]->(p);
MATCH (u:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}), (p:Post {id: "2Z1P61QPX7Q00"}) MERGE (u)-[:TAGGED {tag: "Pubky"}]->(p);
MATCH (u:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}), (p:Post {id: "2Z1P61QPX7Q00"}) MERGE (u)-[:TAGGED {tag: "Pubky"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1P68V42JJ00"}) MERGE (u)-[:TAGGED {tag: "flavio"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1P68V42JJ00"}) MERGE (u)-[:TAGGED {tag: "flavio"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1P68V42JJ00"}) MERGE (u)-[:TAGGED {tag: "flavio"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1P68V42JJ00"}) MERGE (u)-[:TAGGED {tag: "flavio"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1P68V42JJ00"}) MERGE (u)-[:TAGGED {tag: "miguel"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1P68V42JJ00"}) MERGE (u)-[:TAGGED {tag: "miguel"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1P68V42JJ00"}) MERGE (u)-[:TAGGED {tag: "miguel"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1P68V42JJ00"}) MERGE (u)-[:TAGGED {tag: "miguel"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1P68V42JJ00"}) MERGE (u)-[:TAGGED {tag: "🧌"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1P68V42JJ00"}) MERGE (u)-[:TAGGED {tag: "🧌"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1P68V42JJ00"}) MERGE (u)-[:TAGGED {tag: "🧌"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z1P68V42JJ00"}) MERGE (u)-[:TAGGED {tag: "🧌"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z1P778B2G800"}) MERGE (u)-[:TAGGED {tag: "milestone"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z1P778B2G800"}) MERGE (u)-[:TAGGED {tag: "milestone"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z1P778B2G800"}) MERGE (u)-[:TAGGED {tag: "milestone"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z1P778B2G800"}) MERGE (u)-[:TAGGED {tag: "milestone"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z1P778B2G800"}) MERGE (u)-[:TAGGED {tag: "🎉"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1P7H9Y8QV00"}) MERGE (u)-[:TAGGED {tag: "greetings"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1P7H9Y8QV00"}) MERGE (u)-[:TAGGED {tag: "greetings"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1P8ET4Z7T00"}) MERGE (u)-[:TAGGED {tag: "ClickForPoll"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1P8ET4Z7T00"}) MERGE (u)-[:TAGGED {tag: "ClickForPoll"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1P8ET4Z7T00"}) MERGE (u)-[:TAGGED {tag: "Of Course"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1P8ET4Z7T00"}) MERGE (u)-[:TAGGED {tag: "Of Course"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1P8ET4Z7T00"}) MERGE (u)-[:TAGGED {tag: "Yes 100%"}]->(p);
MATCH (u:User {id: "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y"}), (p:Post {id: "2Z1P8ET4Z7T00"}) MERGE (u)-[:TAGGED {tag: "Yes 100%"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1PBYS0F90G0"}) MERGE (u)-[:TAGGED {tag: "#"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1PBYS0F90G0"}) MERGE (u)-[:TAGGED {tag: "#"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1PBYS0F90G0"}) MERGE (u)-[:TAGGED {tag: "# # #"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1PBYS0F90G0"}) MERGE (u)-[:TAGGED {tag: "hashtag"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1PBYS0F90G0"}) MERGE (u)-[:TAGGED {tag: "hashtag"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1PBYS0F90G0"}) MERGE (u)-[:TAGGED {tag: "hashtag"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1PBYS0F90G0"}) MERGE (u)-[:TAGGED {tag: "hashtag"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1PBYS0F90G0"}) MERGE (u)-[:TAGGED {tag: "hashtag"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1PBYS0F90G0"}) MERGE (u)-[:TAGGED {tag: "slashtags"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1PBYS0F90G0"}) MERGE (u)-[:TAGGED {tag: "stashtag"}]->(p);
MATCH (u:User {id: "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o"}), (p:Post {id: "2Z1PBYS0F90G0"}) MERGE (u)-[:TAGGED {tag: "tag"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z23VQHKR6YG0"}) MERGE (u)-[:TAGGED {tag: "truenews"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z23W99C75EG0"}) MERGE (u)-[:TAGGED {tag: "bcashbcashbcash"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z23W99C75EG0"}) MERGE (u)-[:TAGGED {tag: "bcashbcashbcash"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z23W99C75EG0"}) MERGE (u)-[:TAGGED {tag: "bcashbcashbcash"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z23W99C75EG0"}) MERGE (u)-[:TAGGED {tag: "bcashbcashbcash"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z23W99C75EG0"}) MERGE (u)-[:TAGGED {tag: "bcashbcashbcash"}]->(p);
MATCH (u:User {id: "fy3ekk5mfm8nje69nwqb7yhou79kjhm41qp35px8o6zcbqc51k5y"}), (p:Post {id: "2Z25B7X03Q700"}) MERGE (u)-[:TAGGED {tag: "firstpost"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z29ACD50BQ00"}) MERGE (u)-[:TAGGED {tag: "history"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z29ACD50BQ00"}) MERGE (u)-[:TAGGED {tag: "history"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z29ACD50BQ00"}) MERGE (u)-[:TAGGED {tag: "history"}]->(p);
MATCH (u:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}), (p:Post {id: "2Z2R1H784JD00"}) MERGE (u)-[:TAGGED {tag: "Sov"}]->(p);
MATCH (u:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}), (p:Post {id: "2Z2R1H784JD00"}) MERGE (u)-[:TAGGED {tag: "Sov"}]->(p);
MATCH (u:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}), (p:Post {id: "2Z2R1H784JD00"}) MERGE (u)-[:TAGGED {tag: "Sov"}]->(p);
MATCH (u:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}), (p:Post {id: "2Z2R1H784JD00"}) MERGE (u)-[:TAGGED {tag: "Sov"}]->(p);
MATCH (u:User {id: "wyr5x8m3jtrih6di5jwfjgu865t8hg57yo8me3q74mnnpg5gs9yo"}), (p:Post {id: "2Z2R1H784JD00"}) MERGE (u)-[:TAGGED {tag: "Sov"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z3D64GPYF4G0"}) MERGE (u)-[:TAGGED {tag: "whenreplies?"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z3D64GPYF4G0"}) MERGE (u)-[:TAGGED {tag: "whenreplies?"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z3D64GPYF4G0"}) MERGE (u)-[:TAGGED {tag: "whenreplies?"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z3D64GPYF4G0"}) MERGE (u)-[:TAGGED {tag: "whenreplies?"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z3D64GPYF4G0"}) MERGE (u)-[:TAGGED {tag: "whenreplies?"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z3D64GPYF4G0"}) MERGE (u)-[:TAGGED {tag: "whenreplies?"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z3D64GPYF4G0"}) MERGE (u)-[:TAGGED {tag: "💩"}]->(p);
MATCH (u:User {id: "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y"}), (p:Post {id: "2Z3DTV1VRJF00"}) MERGE (u)-[:TAGGED {tag: "fakenews"}]->(p);
MATCH (u:User {id: "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y"}), (p:Post {id: "2Z3DTV1VRJF00"}) MERGE (u)-[:TAGGED {tag: "fakenews"}]->(p);
MATCH (u:User {id: "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y"}), (p:Post {id: "2Z3DTV1VRJF00"}) MERGE (u)-[:TAGGED {tag: "💩💩💩"}]->(p);
MATCH (u:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (p:Post {id: "2Z3GMSDMQJK00"}) MERGE (u)-[:TAGGED {tag: "?"}]->(p);
MATCH (u:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (p:Post {id: "2Z3HDTCCZB500"}) MERGE (u)-[:TAGGED {tag: "hello"}]->(p);
MATCH (u:User {id: "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do"}), (p:Post {id: "2Z3HDTCCZB500"}) MERGE (u)-[:TAGGED {tag: "hello"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z3JXJ4KW4700"}) MERGE (u)-[:TAGGED {tag: "verified"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z3JXJ4KW4700"}) MERGE (u)-[:TAGGED {tag: "verified"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z3JXJ4KW4700"}) MERGE (u)-[:TAGGED {tag: "verified"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z3JXJ4KW4700"}) MERGE (u)-[:TAGGED {tag: "verylongtagsdothistothelayoutshouldwetruncate"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z3JXXZ2AEVG0"}) MERGE (u)-[:TAGGED {tag: "issues"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z3NMPKCJ07G0"}) MERGE (u)-[:TAGGED {tag: "pkarr"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z3NMPKCJ07G0"}) MERGE (u)-[:TAGGED {tag: "pkarr"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z3NTJPPA0D00"}) MERGE (u)-[:TAGGED {tag: "botsarepeopletoo"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z3NTJPPA0D00"}) MERGE (u)-[:TAGGED {tag: "botsarepeopletoo"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z3Q6MJGQXT00"}) MERGE (u)-[:TAGGED {tag: "🫠"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z3Q6MJGQXT00"}) MERGE (u)-[:TAGGED {tag: "🫠"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z4KR850HM100"}) MERGE (u)-[:TAGGED {tag: "slow"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z4KR850HM100"}) MERGE (u)-[:TAGGED {tag: "slow"}]->(p);
MATCH (u:User {id: "dujh8de33hbaqerg1ejyu9ynjh5yqfozm6iexsjdj3h9yfn3n3wy"}), (p:Post {id: "2Z4QSB3MG3300"}) MERGE (u)-[:TAGGED {tag: "test"}]->(p);
MATCH (u:User {id: "dujh8de33hbaqerg1ejyu9ynjh5yqfozm6iexsjdj3h9yfn3n3wy"}), (p:Post {id: "2Z4QSB3MG3300"}) MERGE (u)-[:TAGGED {tag: "test"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z5TYWGC9D700"}) MERGE (u)-[:TAGGED {tag: "🚀"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z5TYWGC9D700"}) MERGE (u)-[:TAGGED {tag: "🚀"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z5TYWGC9D700"}) MERGE (u)-[:TAGGED {tag: "🚀"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z5TYWGC9D700"}) MERGE (u)-[:TAGGED {tag: "🚀"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z5W1DJT1MQG0"}) MERGE (u)-[:TAGGED {tag: "reckless"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z5W7KXCWQK00"}) MERGE (u)-[:TAGGED {tag: "pkarr"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z5W7KXCWQK00"}) MERGE (u)-[:TAGGED {tag: "pkarr"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z5W7KXCWQK00"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z5W7KXCWQK00"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z75P8H2TBK00"}) MERGE (u)-[:TAGGED {tag: "Naiss"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z8M3ZVSVWD00"}) MERGE (u)-[:TAGGED {tag: "🚁"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z8M3ZVSVWD00"}) MERGE (u)-[:TAGGED {tag: "🚁"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z8M3ZVSVWD00"}) MERGE (u)-[:TAGGED {tag: "🚁"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z8M3ZVSVWD00"}) MERGE (u)-[:TAGGED {tag: "🚁"}]->(p);
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "2Z8M80DMTV300"}) MERGE (u)-[:TAGGED {tag: "firstpostagain"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z8M96D3RAR00"}) MERGE (u)-[:TAGGED {tag: "longtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtaglongtag"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z8M96D3RAR00"}) MERGE (u)-[:TAGGED {tag: "pasta"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z8M96D3RAR00"}) MERGE (u)-[:TAGGED {tag: "pasta"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z8M96D3RAR00"}) MERGE (u)-[:TAGGED {tag: "🍝"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z8M96D3RAR00"}) MERGE (u)-[:TAGGED {tag: "🍝"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z8W2AFP242G0"}) MERGE (u)-[:TAGGED {tag: "bitcoin"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z8W2AFP242G0"}) MERGE (u)-[:TAGGED {tag: "bitcoin"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z8W2AFP242G0"}) MERGE (u)-[:TAGGED {tag: "bitcoin"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z8W2AFP242G0"}) MERGE (u)-[:TAGGED {tag: "pizzaday"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z8W2AFP242G0"}) MERGE (u)-[:TAGGED {tag: "pizzaday"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z8W2AFP242G0"}) MERGE (u)-[:TAGGED {tag: "🍕"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z8W2AFP242G0"}) MERGE (u)-[:TAGGED {tag: "🍕"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z91B580Q3300"}) MERGE (u)-[:TAGGED {tag: "always"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z91B580Q3300"}) MERGE (u)-[:TAGGED {tag: "always"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z92MN2S4T9G0"}) MERGE (u)-[:TAGGED {tag: "feature_request"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z92MN2S4T9G0"}) MERGE (u)-[:TAGGED {tag: "feature_request"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z92MN2S4T9G0"}) MERGE (u)-[:TAGGED {tag: "missinghyperlink"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z92MN2S4T9G0"}) MERGE (u)-[:TAGGED {tag: "missinghyperlink"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2Z92MN2S4T9G0"}) MERGE (u)-[:TAGGED {tag: "missinghyperlink"}]->(p);
MATCH (u:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (p:Post {id: "2Z96K6P2RASG0"}) MERGE (u)-[:TAGGED {tag: "testtag"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z97SZCC7PHG0"}) MERGE (u)-[:TAGGED {tag: "Why1USD?"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9D4C5BAJX00"}) MERGE (u)-[:TAGGED {tag: "🧙‍♂️"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9D4C5BAJX00"}) MERGE (u)-[:TAGGED {tag: "🧙‍♂️"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9D6A0GDK900"}) MERGE (u)-[:TAGGED {tag: "synonym"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9DG8TP8E100"}) MERGE (u)-[:TAGGED {tag: "pkarr"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9DG8TP8E100"}) MERGE (u)-[:TAGGED {tag: "pkarr"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9DG8TP8E100"}) MERGE (u)-[:TAGGED {tag: "pkarr"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9GBJ7WCFY00"}) MERGE (u)-[:TAGGED {tag: "🙏"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9GBJ7WCFY00"}) MERGE (u)-[:TAGGED {tag: "🙏"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9GBJ7WCFY00"}) MERGE (u)-[:TAGGED {tag: "🤙"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9GWEBYKY400"}) MERGE (u)-[:TAGGED {tag: "Naiiss"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9GWEBYKY400"}) MERGE (u)-[:TAGGED {tag: "bitcoin"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9GWEBYKY400"}) MERGE (u)-[:TAGGED {tag: "bitcoin"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9GWEBYKY400"}) MERGE (u)-[:TAGGED {tag: "bitcoin"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9GWEBYKY400"}) MERGE (u)-[:TAGGED {tag: "bitcoin"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9GWEBYKY400"}) MERGE (u)-[:TAGGED {tag: "bitcoin"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z9NE6HKC0T00"}) MERGE (u)-[:TAGGED {tag: "pkarr"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z9NE7JQSN900"}) MERGE (u)-[:TAGGED {tag: "👀"}]->(p);
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2Z9NFB8Q1NC00"}) MERGE (u)-[:TAGGED {tag: "fail"}]->(p);
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2Z9NFB8Q1NC00"}) MERGE (u)-[:TAGGED {tag: "fail"}]->(p);
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "2Z9NGRZJ9YC00"}) MERGE (u)-[:TAGGED {tag: "opsec_fail"}]->(p);
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "2Z9NGRZJ9YC00"}) MERGE (u)-[:TAGGED {tag: "pubkypassword"}]->(p);
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "2Z9NGRZJ9YC00"}) MERGE (u)-[:TAGGED {tag: "pubkypassword"}]->(p);
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "2Z9NGRZJ9YC00"}) MERGE (u)-[:TAGGED {tag: "pubkypassword"}]->(p);
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "2Z9NGRZJ9YC00"}) MERGE (u)-[:TAGGED {tag: "pubkypassword"}]->(p);
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2Z9NKTBQF6B00"}) MERGE (u)-[:TAGGED {tag: "123456"}]->(p);
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2Z9NKTBQF6B00"}) MERGE (u)-[:TAGGED {tag: "Jay4President"}]->(p);
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2Z9NKTBQF6B00"}) MERGE (u)-[:TAGGED {tag: "Jay4President"}]->(p);
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2Z9NKTBQF6B00"}) MERGE (u)-[:TAGGED {tag: "Jay4President"}]->(p);
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2Z9NKTBQF6B00"}) MERGE (u)-[:TAGGED {tag: "test"}]->(p);
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2Z9NV94946Y00"}) MERGE (u)-[:TAGGED {tag: "4th_tag"}]->(p);
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2Z9NV94946Y00"}) MERGE (u)-[:TAGGED {tag: "4th_tag"}]->(p);
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2Z9NV94946Y00"}) MERGE (u)-[:TAGGED {tag: "gg"}]->(p);
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2Z9NV94946Y00"}) MERGE (u)-[:TAGGED {tag: "hiring"}]->(p);
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2Z9NV94946Y00"}) MERGE (u)-[:TAGGED {tag: "synonym"}]->(p);
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2Z9NV94946Y00"}) MERGE (u)-[:TAGGED {tag: "💥"}]->(p);
MATCH (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9PHA3X75N00"}) MERGE (u)-[:TAGGED {tag: "bitkit"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9ZHKWFTD800"}) MERGE (u)-[:TAGGED {tag: "gg1"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZA8JHRHXJ600"}) MERGE (u)-[:TAGGED {tag: "gg"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZASN5W6MZFG0"}) MERGE (u)-[:TAGGED {tag: "bitcoin"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZASN5W6MZFG0"}) MERGE (u)-[:TAGGED {tag: "bitcoin"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZATA2F2CVW00"}) MERGE (u)-[:TAGGED {tag: "🤘"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZATA2F2CVW00"}) MERGE (u)-[:TAGGED {tag: "🤘"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZATA2F2CVW00"}) MERGE (u)-[:TAGGED {tag: "🥳"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAV28YDJSXG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAV28YDJSXG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAV28YDJSXG0"}) MERGE (u)-[:TAGGED {tag: "bot"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAV28YDJSXG0"}) MERGE (u)-[:TAGGED {tag: "bot"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAV28YDJSXG0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAV28YDJSXG0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAV8TGM8QB00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAV8TGM8QB00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAV8TGM8QB00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAV8TGM8QB00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2ZAVDJTWS3P00"}) MERGE (u)-[:TAGGED {tag: "Works"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAVFC1DZHPG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAVFC1DZHPG0"}) MERGE (u)-[:TAGGED {tag: "Test🎉"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAVFC1DZHPG0"}) MERGE (u)-[:TAGGED {tag: "Test🎉"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAVFC1DZHPG0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAVNXKHPBH00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAVNXKHPBH00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAVWF4CB5Q00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAVWF4CB5Q00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAVWF4CB5Q00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAW30PTDKBG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAW30PTDKBG0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAW9JBG9YA00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAW9JBG9YA00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWG3STSFH00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWG3STSFH00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWG3STSFH00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWPNE25GD00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWPNE25GD00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWR9RFTYPG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWR9RFTYPG0"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZAWV6RJGHB00"}) MERGE (u)-[:TAGGED {tag: "fixpreview"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZAWV6RJGHB00"}) MERGE (u)-[:TAGGED {tag: "👀"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWVJGFGH400"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWVJGFGH400"}) MERGE (u)-[:TAGGED {tag: "DifficultyAdjustment"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWVJGFGH400"}) MERGE (u)-[:TAGGED {tag: "GracePeriod"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZAWW07HF5MG0"}) MERGE (u)-[:TAGGED {tag: "📷"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZAWW4G4VQSG0"}) MERGE (u)-[:TAGGED {tag: "WhenComments???"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWX6X2E1G00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAWX6X2E1G00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2ZAWY8KSKV600"}) MERGE (u)-[:TAGGED {tag: "NoComments😭"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAX0FNTWXA00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAX0FNTWXA00"}) MERGE (u)-[:TAGGED {tag: "LightningNetwork"}]->(p);
MATCH (u:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (p:Post {id: "2ZAX1DBDD5YG0"}) MERGE (u)-[:TAGGED {tag: "killdozer"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAX3RG4X0D00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAX3RG4X0D00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXBYD491Q00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXBYD491Q00"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZAXC20DJG400"}) MERGE (u)-[:TAGGED {tag: "🍷"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXGVGPD97G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXGVGPD97G0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZAXJWQJ71Z00"}) MERGE (u)-[:TAGGED {tag: "🖥️"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXQD2WYZJG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXQD2WYZJG0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZAXRP9Y7M500"}) MERGE (u)-[:TAGGED {tag: "standup-and-walk"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXS1G7H4WG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXS1G7H4WG0"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXXYM0Z7MG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAXXYM0Z7MG0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAY4G649JZ00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAY4G649JZ00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYB1Q0G22G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYB1Q0G22G0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYHK9CJ4AG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYHK9CJ4AG0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYPGE6MHYG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYPGE6MHYG0"}) MERGE (u)-[:TAGGED {tag: "DifficultyAdjustment"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYPGE6MHYG0"}) MERGE (u)-[:TAGGED {tag: "GracePeriod"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYR4T072DG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYR4T072DG0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYSS7SPMDG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYSS7SPMDG0"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYVDM48TCG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYVDM48TCG0"}) MERGE (u)-[:TAGGED {tag: "LightningNetwork"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYYPCEEWWG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAYYPCEEWWG0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZ57X95J500"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZ57X95J500"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZ6WB0WNEG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZ6WB0WNEG0"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZBSFANCN00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZBSFANCN00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZJB037XFG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZJB037XFG0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZRWNFQGDG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZRWNFQGDG0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZZE6EDRQ00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZAZZE6EDRQ00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB05ZR9VTK00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB05ZR9VTK00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB0CHACWC6G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB0CHACWC6G0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB0K2V4Y6KG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB0K2V4Y6KG0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZB0MR4ZFYB00"}) MERGE (u)-[:TAGGED {tag: "fgjhdfldfh"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZB0MR4ZFYB00"}) MERGE (u)-[:TAGGED {tag: "fhddfhdfh"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZB0MR4ZFYB00"}) MERGE (u)-[:TAGGED {tag: "hfddfhdfh"}]->(p);
MATCH (u:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (p:Post {id: "2ZB0PB2KVV700"}) MERGE (u)-[:TAGGED {tag: "lol"}]->(p);
MATCH (u:User {id: "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y"}), (p:Post {id: "2ZB0PB2KVV700"}) MERGE (u)-[:TAGGED {tag: "lol"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB0SMCYX7J00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB0SMCYX7J00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB105Y98Q0G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB105Y98Q0G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB105Y98Q0G0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB16QG14K1G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB16QG14K1G0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1D91D0AAG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1D91D0AAG0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1KTN60RN00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1KTN60RN00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1NF3BF7GG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1NF3BF7GG0"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1RQTAXFR00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1RQTAXFR00"}) MERGE (u)-[:TAGGED {tag: "DifficultyAdjustment"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1RQTAXFR00"}) MERGE (u)-[:TAGGED {tag: "GracePeriod"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1TC6J2KK00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1TC6J2KK00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZB1W7ZPCGHG0"}) MERGE (u)-[:TAGGED {tag: "🤖"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZB1W7ZPCGHG0"}) MERGE (u)-[:TAGGED {tag: "🤖"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1XN0942200"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB1XN0942200"}) MERGE (u)-[:TAGGED {tag: "LightningNetwork"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB20XR1N4D00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZB20XR1N4D00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "ym1rn4rfjg8857y13uuehc4tw6sfqtzpu881ycdj7iiyo6kqsspy"}), (p:Post {id: "2ZB26RT500X00"}) MERGE (u)-[:TAGGED {tag: "🤘"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZB9WRZ6B6200"}) MERGE (u)-[:TAGGED {tag: "💯"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZB9WRZ6B6200"}) MERGE (u)-[:TAGGED {tag: "💯"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZB9WRZ6B6200"}) MERGE (u)-[:TAGGED {tag: "💯"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZBAKNPJ2D900"}) MERGE (u)-[:TAGGED {tag: "nothingisreal"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZBAKNPJ2D900"}) MERGE (u)-[:TAGGED {tag: "nothingisreal"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZBCBSQPKGGG0"}) MERGE (u)-[:TAGGED {tag: "🤣"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZBCBSQPKGGG0"}) MERGE (u)-[:TAGGED {tag: "🤣"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZBFGACDZQR00"}) MERGE (u)-[:TAGGED {tag: "💯"}]->(p);
MATCH (u:User {id: "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y"}), (p:Post {id: "2ZBFXCKJ48RG0"}) MERGE (u)-[:TAGGED {tag: "bad"}]->(p);
MATCH (u:User {id: "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y"}), (p:Post {id: "2ZBFXCKJ48RG0"}) MERGE (u)-[:TAGGED {tag: "bad"}]->(p);
MATCH (u:User {id: "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y"}), (p:Post {id: "2ZBFXCKJ48RG0"}) MERGE (u)-[:TAGGED {tag: "👀"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZBKXMD487E00"}) MERGE (u)-[:TAGGED {tag: "based"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZBKXMD487E00"}) MERGE (u)-[:TAGGED {tag: "based"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZC99Q5B4TS00"}) MERGE (u)-[:TAGGED {tag: "gg1"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZCE7DGVB96G0"}) MERGE (u)-[:TAGGED {tag: "gg"}]->(p);
MATCH (u:User {id: "z88bknbxf3q8rxcaehp5strbjgqmyt84yskd4cw7t56rpmakwa8o"}), (p:Post {id: "2ZCEBM3A0Z900"}) MERGE (u)-[:TAGGED {tag: "dfhdflhjjlkdfjhkljdf"}]->(p);
MATCH (u:User {id: "z88bknbxf3q8rxcaehp5strbjgqmyt84yskd4cw7t56rpmakwa8o"}), (p:Post {id: "2ZCEBM3A0Z900"}) MERGE (u)-[:TAGGED {tag: "fhdkjhjdfklhjldfhkld"}]->(p);
MATCH (u:User {id: "z88bknbxf3q8rxcaehp5strbjgqmyt84yskd4cw7t56rpmakwa8o"}), (p:Post {id: "2ZCEBM3A0Z900"}) MERGE (u)-[:TAGGED {tag: "sdgdfhdfhdfhjdfjlhjl"}]->(p);
MATCH (u:User {id: "jes1c7hjkw8d7osas7ai9jbqfz6r5i15e7rh1zih5wnm7mknu5do"}), (p:Post {id: "2ZCEFB0KYPGG0"}) MERGE (u)-[:TAGGED {tag: "gg1"}]->(p);
MATCH (u:User {id: "jes1c7hjkw8d7osas7ai9jbqfz6r5i15e7rh1zih5wnm7mknu5do"}), (p:Post {id: "2ZCEFB0KYPGG0"}) MERGE (u)-[:TAGGED {tag: "gg2"}]->(p);
MATCH (u:User {id: "jes1c7hjkw8d7osas7ai9jbqfz6r5i15e7rh1zih5wnm7mknu5do"}), (p:Post {id: "2ZCEFB0KYPGG0"}) MERGE (u)-[:TAGGED {tag: "gg3"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZCW1TGR5BKG0"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZCW1TGR5BKG0"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZCW1TGR5BKG0"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZCW1TGR5BKG0"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZCW1TGR5BKG0"}) MERGE (u)-[:TAGGED {tag: "pubky"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD19WPE9GC00"}) MERGE (u)-[:TAGGED {tag: "🤘"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD1BGEH4M000"}) MERGE (u)-[:TAGGED {tag: "🥲"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD1BMZB80PG0"}) MERGE (u)-[:TAGGED {tag: "aimusic"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZD2GBAGJ4XG0"}) MERGE (u)-[:TAGGED {tag: "elonmusk"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZD2GBAGJ4XG0"}) MERGE (u)-[:TAGGED {tag: "elonmusk"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZD2GBAGJ4XG0"}) MERGE (u)-[:TAGGED {tag: "superapp"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZD2GBAGJ4XG0"}) MERGE (u)-[:TAGGED {tag: "superapp"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZD2GBAGJ4XG0"}) MERGE (u)-[:TAGGED {tag: "x"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD2YWZJ0RMG0"}) MERGE (u)-[:TAGGED {tag: "👍"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD2YWZJ0RMG0"}) MERGE (u)-[:TAGGED {tag: "👍"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD2YWZJ0RMG0"}) MERGE (u)-[:TAGGED {tag: "👍"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD2YWZJ0RMG0"}) MERGE (u)-[:TAGGED {tag: "👍"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD2YWZJ0RMG0"}) MERGE (u)-[:TAGGED {tag: "👎"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD2YWZJ0RMG0"}) MERGE (u)-[:TAGGED {tag: "👎"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD2YWZJ0RMG0"}) MERGE (u)-[:TAGGED {tag: "👎"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZD52PVKVSY00"}) MERGE (u)-[:TAGGED {tag: "😶‍🌫️"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZD54TZR9XS00"}) MERGE (u)-[:TAGGED {tag: "FUD"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZD54TZR9XS00"}) MERGE (u)-[:TAGGED {tag: "FUD"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD57HSGPJ300"}) MERGE (u)-[:TAGGED {tag: "NervousHaha"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD57HSGPJ300"}) MERGE (u)-[:TAGGED {tag: "NervousHaha"}]->(p);
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2ZD5RQT277A00"}) MERGE (u)-[:TAGGED {tag: "☮️"}]->(p);
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2ZD5RQT277A00"}) MERGE (u)-[:TAGGED {tag: "☮️"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD6BK61CRT00"}) MERGE (u)-[:TAGGED {tag: "🍄"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD6BK61CRT00"}) MERGE (u)-[:TAGGED {tag: "🍄"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD6BK61CRT00"}) MERGE (u)-[:TAGGED {tag: "🍄"}]->(p);
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2ZD6CPRQTAC00"}) MERGE (u)-[:TAGGED {tag: "🐸"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZD6CZS8Z0SG0"}) MERGE (u)-[:TAGGED {tag: "🧙‍♂️"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZD6CZS8Z0SG0"}) MERGE (u)-[:TAGGED {tag: "🧙‍♂️"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZD6CZS8Z0SG0"}) MERGE (u)-[:TAGGED {tag: "🧙‍♂️"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZD6CZS8Z0SG0"}) MERGE (u)-[:TAGGED {tag: "🧙‍♂️"}]->(p);
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2ZD6EH3PEF000"}) MERGE (u)-[:TAGGED {tag: "🇫🇷"}]->(p);
MATCH (u:User {id: "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo"}), (p:Post {id: "2ZD6EH3PEF000"}) MERGE (u)-[:TAGGED {tag: "🇫🇷"}]->(p);
MATCH (u:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (p:Post {id: "2ZD6M4WB0X6G0"}) MERGE (u)-[:TAGGED {tag: "🥳"}]->(p);
MATCH (u:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (p:Post {id: "2ZD6M4WB0X6G0"}) MERGE (u)-[:TAGGED {tag: "🥳"}]->(p);
MATCH (u:User {id: "sazaz5yowshbkfbsqi4n47rbyn8xbmokmu8kft59iyjazjxo4jeo"}), (p:Post {id: "2ZD6M4WB0X6G0"}) MERGE (u)-[:TAGGED {tag: "🥳"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD9VJK97YN00"}) MERGE (u)-[:TAGGED {tag: "linux"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD9VJK97YN00"}) MERGE (u)-[:TAGGED {tag: "linux"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD9VJK97YN00"}) MERGE (u)-[:TAGGED {tag: "phones"}]->(p);
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "2ZDDXBWR520G0"}) MERGE (u)-[:TAGGED {tag: "🤣"}]->(p);
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "2ZDDXBWR520G0"}) MERGE (u)-[:TAGGED {tag: "🤣"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZDH5QAWVHT00"}) MERGE (u)-[:TAGGED {tag: "🅰️"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZDH5QAWVHT00"}) MERGE (u)-[:TAGGED {tag: "🅰️"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZDH5QAWVHT00"}) MERGE (u)-[:TAGGED {tag: "🅰️"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZDH5QAWVHT00"}) MERGE (u)-[:TAGGED {tag: "🅰️"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZDH5QAWVHT00"}) MERGE (u)-[:TAGGED {tag: "🅰️"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZDH5QAWVHT00"}) MERGE (u)-[:TAGGED {tag: "🅱️"}]->(p);
MATCH (u:User {id: "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"}), (p:Post {id: "2ZDMBYCN25X00"}) MERGE (u)-[:TAGGED {tag: "😎"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN0W875TJ00"}) MERGE (u)-[:TAGGED {tag: "its_fine_🔥"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN0W875TJ00"}) MERGE (u)-[:TAGGED {tag: "its_fine_🔥"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN0W875TJ00"}) MERGE (u)-[:TAGGED {tag: "its_fine_🔥"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN19JTA2B00"}) MERGE (u)-[:TAGGED {tag: "EventsGood"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDPGX8W3DTG0"}) MERGE (u)-[:TAGGED {tag: "🤖"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDPHVBK54XG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDPHVBK54XG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDPHVBK54XG0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDPHVBK54XG0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDPQYABQQQ00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDPQYABQQQ00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDPYFYPYTVG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDPYFYPYTVG0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQ3D3EKP700"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQ3D3EKP700"}) MERGE (u)-[:TAGGED {tag: "DifficultyAdjustment"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQ3D3EKP700"}) MERGE (u)-[:TAGGED {tag: "GracePeriod"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQ51DAF22G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQ51DAF22G0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQ6NZTPNQ00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQ6NZTPNQ00"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQ8A9MQW8G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQ8A9MQW8G0"}) MERGE (u)-[:TAGGED {tag: "LightningNetwork"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQBK17ZQ3G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQBK17ZQ3G0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQJ4GXET7G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQJ4GXET7G0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQJ4GXET7G0"}) MERGE (u)-[:TAGGED {tag: "spam"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQKS2KMQ000"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQKS2KMQ000"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQKS2KMQ000"}) MERGE (u)-[:TAGGED {tag: "spam"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQRP4YNY6G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQRP4YNY6G0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQRP4YNY6G0"}) MERGE (u)-[:TAGGED {tag: "spam"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQZ7KMAVC00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQZ7KMAVC00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDQZ7KMAVC00"}) MERGE (u)-[:TAGGED {tag: "spam"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDR5S6HTRQ00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDR5S6HTRQ00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDR5S6HTRQ00"}) MERGE (u)-[:TAGGED {tag: "spam"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRCAPMQAK00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRCAPMQAK00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRCAPMQAK00"}) MERGE (u)-[:TAGGED {tag: "spam"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRJW8TVK0G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRJW8TVK0G0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRJW8TVK0G0"}) MERGE (u)-[:TAGGED {tag: "spam"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRSDSNTH400"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRSDSNTH400"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRSDSNTH400"}) MERGE (u)-[:TAGGED {tag: "spam"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRZZCEDAF00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRZZCEDAF00"}) MERGE (u)-[:TAGGED {tag: "bot"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRZZCEDAF00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDRZZCEDAF00"}) MERGE (u)-[:TAGGED {tag: "spam"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDS6GWNSDDG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDS6GWNSDDG0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDS6GWNSDDG0"}) MERGE (u)-[:TAGGED {tag: "spam"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDSD2F1V6J00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDSD2F1V6J00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDSKKZWJQJG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDSKKZWJQJG0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDST5J63VC00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDST5J63VC00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDT0Q33GBB00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDT0Q33GBB00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDT2BHYHD5G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDT2BHYHD5G0"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDT5MAJJ55G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDT5MAJJ55G0"}) MERGE (u)-[:TAGGED {tag: "DifficultyAdjustment"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDT5MAJJ55G0"}) MERGE (u)-[:TAGGED {tag: "GracePeriod"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDT78ND8T500"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDT78ND8T500"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTAHEV6YR00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTAHEV6YR00"}) MERGE (u)-[:TAGGED {tag: "LightningNetwork"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTDT694BQG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTDT694BQG0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTMBRKJV700"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTMBRKJV700"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTP06PKKTG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTP06PKKTG0"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTTX9HKAT00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDTTX9HKAT00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDV1EW195M00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDV1EW195M00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDV33AYH4XG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDV33AYH4XG0"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDV80CDGWC00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDV80CDGWC00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDVEJ08D0Y00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDVEJ08D0Y00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDVN3FNZHA00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDVN3FNZHA00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDVVN3JX77G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDVVN3JX77G0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW0J90AN000"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW0J90AN000"}) MERGE (u)-[:TAGGED {tag: "DifficultyAdjustment"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW0J90AN000"}) MERGE (u)-[:TAGGED {tag: "GracePeriod"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW26N6BG3G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW26N6BG3G0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW3V5Y7W000"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW3V5Y7W000"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW5FEHH6C00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW5FEHH6C00"}) MERGE (u)-[:TAGGED {tag: "LightningNetwork"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW8R72ANMG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDW8R72ANMG0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDWF9PVQ4SG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDWF9PVQ4SG0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDWGY905Z4G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDWGY905Z4G0"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDWNV9TDMT00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDWNV9TDMT00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDWWCSRRZJG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDWWCSRRZJG0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDX2YBC5Y7G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDX2YBC5Y7G0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDX9FWEXSW00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDX9FWEXSW00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDXG1EB5TZ00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDXG1EB5TZ00"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDXPJZ3SB600"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDXPJZ3SB600"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDXPJZ3SB600"}) MERGE (u)-[:TAGGED {tag: "test"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDXX4HE9DPG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDXX4HE9DPG0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDY3P2P13RG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDY3P2P13RG0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYA7MH312G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYA7MH312G0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYGS5S86D00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYGS5S86D00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYGS5S86D00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYQAQFA74G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYQAQFA74G0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYQAQFA74G0"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYXW8751NG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYXW8751NG0"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYZGQ4XKTG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDYZGQ4XKTG0"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZ2SF29HK00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZ2SF29HK00"}) MERGE (u)-[:TAGGED {tag: "DifficultyAdjustment"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZ2SF29HK00"}) MERGE (u)-[:TAGGED {tag: "GracePeriod"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZ4DTKRJ900"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZ4DTKRJ900"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZ4DTKRJ900"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZ7PM0JVK00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZ7PM0JVK00"}) MERGE (u)-[:TAGGED {tag: "LightningNetwork"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDZ87NZ4H700"}) MERGE (u)-[:TAGGED {tag: "dev"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZHGVTQV600"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZHGVTQV600"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZHGVTQV600"}) MERGE (u)-[:TAGGED {tag: "fees"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZK595DDRG0"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZK595DDRG0"}) MERGE (u)-[:TAGGED {tag: "Halving"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZR2G775W00"}) MERGE (u)-[:TAGGED {tag: "Bitcoin"}]->(p);
MATCH (u:User {id: "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o"}), (p:Post {id: "2ZDZR2G775W00"}) MERGE (u)-[:TAGGED {tag: "price"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZE41GVGAZV00"}) MERGE (u)-[:TAGGED {tag: "ISeeYouReply"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZE8P6VPBVW00"}) MERGE (u)-[:TAGGED {tag: "Bitkit"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZE8P6VPBVW00"}) MERGE (u)-[:TAGGED {tag: "Bitkit"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZE8P6VPBVW00"}) MERGE (u)-[:TAGGED {tag: "Bitkit"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZE8P6VPBVW00"}) MERGE (u)-[:TAGGED {tag: "Bitkit"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZEBH4J0K4G00"}) MERGE (u)-[:TAGGED {tag: "1️⃣"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZEBH4J0K4G00"}) MERGE (u)-[:TAGGED {tag: "1️⃣"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:TAGGED {tag: "vs"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:TAGGED {tag: "vs"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:TAGGED {tag: "vs"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:TAGGED {tag: "vs"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:TAGGED {tag: "🍝"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:TAGGED {tag: "🍝"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:TAGGED {tag: "🍝"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:TAGGED {tag: "🧀"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:TAGGED {tag: "🧀"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:TAGGED {tag: "🧀"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECXVXHZBE00"}) MERGE (u)-[:TAGGED {tag: "PreprareToDie"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECXVXHZBE00"}) MERGE (u)-[:TAGGED {tag: "PreprareToDie"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECXVXHZBE00"}) MERGE (u)-[:TAGGED {tag: "PreprareToDie"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZEEM0CKE3CG0"}) MERGE (u)-[:TAGGED {tag: "What"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZEEM0CKE3CG0"}) MERGE (u)-[:TAGGED {tag: "What"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZEEM0CKE3CG0"}) MERGE (u)-[:TAGGED {tag: "What"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZEEM0CKE3CG0"}) MERGE (u)-[:TAGGED {tag: "fuckyeah"}]->(p);
MATCH (u:User {id: "3kahfdtt8qs7rmtxntx4nxcbb4jifwzykxp1g6uj5my1y7fcf9ro"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9Z8PXK5WBG0"}]->(p);
MATCH (u:User {id: "3qgon1apkcmp63xbqpkrb3zzrja3nq9wou4u5bf7uu8rc9ehfo3y"}), (p:Post {id: "2ZCCYNPWSDNG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZCCYWYKKZA00"}]->(p);
MATCH (u:User {id: "77m8jyqqrd41xzu5b67m8z5wuypuatc54gmw5gcax6ae3yeca6wo"}), (p:Post {id: "0RE3WS2NPCP0"}) MERGE (u)-[:BOOKMARKED {id: "0RE3WYPE3HTG"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9PFG3B1EPG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9PFGC3WWBG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9PFGKGVHM00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZEPP6ANZG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZF393RAM00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZFR3DC4100"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZFR4PW0PG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZFR5C37AG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZFR5Y6NT00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZFR6K7K0G0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZFR7MNSV00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZFR8752QG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZFSGSY6SG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZGG113BRG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9P8AN738C00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZGRZTJZJ00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9ZHKWFTD800"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZK5HQV5V00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9ZHKWFTD800"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZK6P0EG300"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9ZHKWFTD800"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZK6Y7A9B00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9ZHKWFTD800"}) MERGE (u)-[:BOOKMARKED {id: "2Z9ZMX8J9QC00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZAXC20DJG400"}) MERGE (u)-[:BOOKMARKED {id: "2ZAXCRB2200G0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZAXC20DJG400"}) MERGE (u)-[:BOOKMARKED {id: "2ZAXCRHG08P00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZAXC20DJG400"}) MERGE (u)-[:BOOKMARKED {id: "2ZAXCS3Z38700"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZB0MR4ZFYB00"}) MERGE (u)-[:BOOKMARKED {id: "2ZB15QRT0P400"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZCD6J45SPR00"}) MERGE (u)-[:BOOKMARKED {id: "2ZCD6WJB2HA00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZCE66E25A600"}) MERGE (u)-[:BOOKMARKED {id: "2ZCE66XPDP800"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZCK5N1QJCHG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZCK5PDFXTKG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZCK5N1QJCHG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZCK5PQ2PJB00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZCK6P4261CG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZCK6RWX0MYG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZCK6P4261CG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZCK6S7DBE4G0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZCK6P4261CG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZCK722J91K00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZCK6TB1K0NG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZCK6WFFCX5G0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZD0PF5XY4600"}) MERGE (u)-[:BOOKMARKED {id: "2ZD0QJT6P7TG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZD1N7WRVKG00"}) MERGE (u)-[:BOOKMARKED {id: "2ZE8PJ4FX9T00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZD6JHJQ6MZG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZDAJ34ET27G0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZE8P6VPBVW00"}) MERGE (u)-[:BOOKMARKED {id: "2ZE9AV197PM00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:BOOKMARKED {id: "2ZEEBB75CRMG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:BOOKMARKED {id: "2ZEEBKAKHEB00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZEHQTM6V68G0"}) MERGE (u)-[:BOOKMARKED {id: "2ZEHQX204Y3G0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZEHSGPKTV5G0"}) MERGE (u)-[:BOOKMARKED {id: "2ZEHTWV17C2G0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZF0EVMASE500"}) MERGE (u)-[:BOOKMARKED {id: "2ZF0EVW37ES00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZF0EVMASE500"}) MERGE (u)-[:BOOKMARKED {id: "2ZF0EW9W1ECG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZF0EVMASE500"}) MERGE (u)-[:BOOKMARKED {id: "2ZF0EWEEK1H00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZF0MXSRN4C00"}) MERGE (u)-[:BOOKMARKED {id: "2ZF0MY0PY6S00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZF0MXSRN4C00"}) MERGE (u)-[:BOOKMARKED {id: "2ZF0MY5KZWA00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZF0MXSRN4C00"}) MERGE (u)-[:BOOKMARKED {id: "2ZF0MYB3T5TG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZF0MXSRN4C00"}) MERGE (u)-[:BOOKMARKED {id: "2ZF0MYFBE5700"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZF0P5761NR00"}) MERGE (u)-[:BOOKMARKED {id: "2ZF0P5TQAQ200"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZF55MP6CJDG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZF55NAKQ6W00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZF55MP6CJDG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZF55PCQP1100"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZF55MP6CJDG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZF55RBT2JH00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9NGRZJ9YC00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9Z96M09G7G0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9NGRZJ9YC00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9Z96V40H1G0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9NGRZJ9YC00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9Z97CTDW100"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9NFB8Q1NC00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9NFRZFMGV00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2Z9NFB8Q1NC00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9NGB686SA00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZAVDJTWS3P00"}) MERGE (u)-[:BOOKMARKED {id: "2ZAVNQBC72DG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZAVFC1DZHPG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZAVNQJJES0G0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZDZR2G775W00"}) MERGE (u)-[:BOOKMARKED {id: "2ZE4190WM9700"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZDZR2G775W00"}) MERGE (u)-[:BOOKMARKED {id: "2ZE41T8HW3TG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZDZR2G775W00"}) MERGE (u)-[:BOOKMARKED {id: "2ZE42EQCV1JG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZC99Q5B4TS00"}) MERGE (u)-[:BOOKMARKED {id: "2ZC9E3B08Q5G0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZD9VJK97YN00"}) MERGE (u)-[:BOOKMARKED {id: "2ZDACEJW4ZPG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZCA9DZ6SZMG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZCD758KNQV00"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZCA9DZ6SZMG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZCE6T8AY53G0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZAVYKGT508G0"}) MERGE (u)-[:BOOKMARKED {id: "2ZAW3W28ME2G0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZD6CPRQTAC00"}) MERGE (u)-[:BOOKMARKED {id: "2ZD6CPTA2ZDG0"}]->(p);
MATCH (u:User {id: "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy"}), (p:Post {id: "2ZD6CPRQTAC00"}) MERGE (u)-[:BOOKMARKED {id: "2ZD6CQ4PKA4G0"}]->(p);
MATCH (u:User {id: "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty"}), (p:Post {id: "0RE3ZD46ZB3G"}) MERGE (u)-[:BOOKMARKED {id: "0RE3ZPA663C0"}]->(p);
MATCH (u:User {id: "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o"}), (p:Post {id: "2Z9NFB8Q1NC00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9NK0A055N00"}]->(p);
MATCH (u:User {id: "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao"}), (p:Post {id: "2Z9NGRZJ9YC00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9NH248YXEG0"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZAX1DBDD5YG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZAX3R31BFAG0"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZAX1DBDD5YG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZAX3SE2N5000"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD6JHJQ6MZG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZD6QRZ7EZ2G0"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:BOOKMARKED {id: "2ZEE7XZX34Q00"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZDZ4DTKRJ900"}) MERGE (u)-[:BOOKMARKED {id: "2ZDZX5TYG3NG0"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZDZ7PM0JVK00"}) MERGE (u)-[:BOOKMARKED {id: "2ZDZX5RMTTA00"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZDZHGVTQV600"}) MERGE (u)-[:BOOKMARKED {id: "2ZDZX5KPGPV00"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZDZK595DDRG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZDZX5H2CNT00"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZDZR2G775W00"}) MERGE (u)-[:BOOKMARKED {id: "2ZDZX5C9AZR00"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2Z9GWEBYKY400"}) MERGE (u)-[:BOOKMARKED {id: "2Z9NE6Y5ZS300"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZD6BK61CRT00"}) MERGE (u)-[:BOOKMARKED {id: "2ZD6R7T96PB00"}]->(p);
MATCH (u:User {id: "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo"}), (p:Post {id: "2ZDZ87NZ4H700"}) MERGE (u)-[:BOOKMARKED {id: "2ZDZX5P4QQ900"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDDYCKG90H00"}) MERGE (u)-[:BOOKMARKED {id: "2ZDNWDBCW7QG0"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZCVS8MYJXH00"}) MERGE (u)-[:BOOKMARKED {id: "2ZCW0GEZW2BG0"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDSBY99RAZ00"}) MERGE (u)-[:BOOKMARKED {id: "0RDTGQTJV9T0"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2Z9NE6HKC0T00"}) MERGE (u)-[:BOOKMARKED {id: "2Z9NJS9EXB700"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZD9VJK97YN00"}) MERGE (u)-[:BOOKMARKED {id: "2ZD9VQ0ASXVG0"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDMZ753WA2G0"}) MERGE (u)-[:BOOKMARKED {id: "2ZDMZBSG7ARG0"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDMZ753WA2G0"}) MERGE (u)-[:BOOKMARKED {id: "2ZDNVTWVPKQG0"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN0W875TJ00"}) MERGE (u)-[:BOOKMARKED {id: "2ZDN0Y5WZNRG0"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN15FY672G0"}) MERGE (u)-[:BOOKMARKED {id: "2ZDNVP0TZEX00"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN15FY672G0"}) MERGE (u)-[:BOOKMARKED {id: "2ZDNW9J6PK9G0"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN15FY672G0"}) MERGE (u)-[:BOOKMARKED {id: "2ZDNWA30559G0"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN15FY672G0"}) MERGE (u)-[:BOOKMARKED {id: "2ZDNWAGJ8CB00"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN15FY672G0"}) MERGE (u)-[:BOOKMARKED {id: "2ZDNWB0KT3800"}]->(p);
MATCH (u:User {id: "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"}), (p:Post {id: "2ZDN1KG9QXKG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZDNVWCKNFZ00"}]->(p);
MATCH (u:User {id: "qnostbczowotpm8twnhjnzruyebjrpyfkf7ehxd1uxqgnjw1aqpo"}), (p:Post {id: "2ZD58WJ3EK900"}) MERGE (u)-[:BOOKMARKED {id: "2ZD590W150600"}]->(p);
MATCH (u:User {id: "qnostbczowotpm8twnhjnzruyebjrpyfkf7ehxd1uxqgnjw1aqpo"}), (p:Post {id: "2ZD58WJ3EK900"}) MERGE (u)-[:BOOKMARKED {id: "2ZD591ATV2S00"}]->(p);
MATCH (u:User {id: "qnostbczowotpm8twnhjnzruyebjrpyfkf7ehxd1uxqgnjw1aqpo"}), (p:Post {id: "2ZD52PVKVSY00"}) MERGE (u)-[:BOOKMARKED {id: "2ZD590ZZRK2G0"}]->(p);
MATCH (u:User {id: "qnostbczowotpm8twnhjnzruyebjrpyfkf7ehxd1uxqgnjw1aqpo"}), (p:Post {id: "2ZD54TZR9XS00"}) MERGE (u)-[:BOOKMARKED {id: "2ZD58Y3AGNG00"}]->(p);
MATCH (u:User {id: "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy"}), (p:Post {id: "2ZECRNM66G900"}) MERGE (u)-[:BOOKMARKED {id: "2ZEHPZQW7EY00"}]->(p);
MATCH (u:User {id: "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy"}), (p:Post {id: "2ZEEM0CKE3CG0"}) MERGE (u)-[:BOOKMARKED {id: "2ZEHPZZM6ZPG0"}]->(p);
MATCH (u:User {id: "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy"}), (p:Post {id: "2ZEHNJ2CCW500"}) MERGE (u)-[:BOOKMARKED {id: "2ZEHNKK56GN00"}]->(p);
MATCH (u:User {id: "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy"}), (p:Post {id: "2ZEHNPE26FB00"}) MERGE (u)-[:BOOKMARKED {id: "2ZEHNPP9544G0"}]->(p);
MATCH (u:User {id: "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy"}), (p:Post {id: "2ZEHNPE26FB00"}) MERGE (u)-[:BOOKMARKED {id: "2ZEHNQC1S2E00"}]->(p);
MATCH (u:User {id: "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy"}), (p:Post {id: "2ZEHNTY01D600"}) MERGE (u)-[:BOOKMARKED {id: "2ZEHNV9D4NY00"}]->(p);
MATCH (u:User {id: "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy"}), (p:Post {id: "2ZEHNTY01D600"}) MERGE (u)-[:BOOKMARKED {id: "2ZEHNVC975100"}]->(p);
MATCH (u:User {id: "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy"}), (p:Post {id: "2ZEHNTY01D600"}) MERGE (u)-[:BOOKMARKED {id: "2ZEHNVF5TZJ00"}]->(p);
MATCH (u:User {id: "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy"}), (p:Post {id: "2ZEHNTY01D600"}) MERGE (u)-[:BOOKMARKED {id: "2ZEHNVJQECJG0"}]->(p);
MATCH (u:User {id: "y4q8yahtdp6qqu8tzsde83p5zagnzou5cagq9jpt74df67wdt4to"}), (p:Post {id: "2ZDN15FY672G0"}) MERGE (u)-[:BOOKMARKED {id: "2ZDGH298E3800"}]->(p);