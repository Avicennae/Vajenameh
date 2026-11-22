-- phpMyAdmin SQL Dump (mini fixture for tests; same shape as the real dump)
SET SQL_MODE = "NO_AUTO_VALUE_ON_ZERO";
START TRANSACTION;
/*!40101 SET NAMES utf8mb4 */;

CREATE TABLE `words` (
  `id` int(255) NOT NULL,
  `word` text COLLATE utf8mb4_persian_ci NOT NULL,
  `meaning` text COLLATE utf8mb4_persian_ci NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_persian_ci;

INSERT INTO `words` (`id`, `word`, `meaning`) VALUES
(1, 'آب', '<span class=\"hlight\">آب .</span> (اِ) مایعی شفاف بی مَزه و بوی. <BR><font color=\"Blue\">◄</font> دریا. بحر. <br />بدانگه کجا خواست بگذاشت آب <br />به پیران چنین گفت افراسیاب .<br /> <font color=\"Red\">فردوسی .</font><br /></font></div>'),
(2, 'آب', '<span class=\"hlight\">آب .</span> (اِ) نام ماه یازدهم از سال ملی یهود، غُرّه ٔ آن با مرداد مطابق است.<br /></font></div>'),
(3, 'آب', '<span class=\"hlight\">آب .</span> (اِ) رونق و جلا. آب شمشیر.<br /></font></div>'),
(4, 'اب', '<span class=\"hlight\">اب .</span> (ع اِ) میوه و چراگاه.<br /></font></div>'),
(5, 'آباد', 'آباد. (ص) معمور، پرنعمت. مقابل ویران.<br /></font></div>'),
(6, 'آبادان', 'آبادان. (اِخ) شهری در خوزستان و کنار اروندرود.<br /></font></div>'),
(7, 'آب آسیا', '<span class=\"hlight\">آب آسیا.</span> (اِ مرکب ) آسیا که بزورِ آب گردد.<br /></font></div>'),
(8, 'آسیاب', 'آسیاب. (اِ مرکب) جای آرد کردن گندم. آسیا.<br /></font></div>'),
(9, 'ابن مقلة', 'ابن مقلة. (اِخ) وزیر عباسی و خطاط مشهور، مبدع خط نسخ.<br /></font></div>'),
(10, 'بنی امیّه', 'بنی امیّه. (اِخ) خاندان خلفای اموی.<br /></font></div>'),
(11, 'کتاب', 'کتاب. (ع اِ) نوشته. <font color=\"DarkGreen\"><b>[ کِتاب ]</font></b> مجموعه ای از اوراق.<br /></font></div>'),
(12, 'کتابخانه', 'کتابخانه. (اِ مرکب) جای نگهداری کتاب ها. خط و نسخ.<br /></font></div>'),
(13, 'کتابفروش', 'کتابفروش. (ص مرکب) فروشنده ٔ کتاب.<br /></font></div>'),
(14, 'کار', 'کار. (اِ) عمل و پیشه.<br /></font></div>'),
(15, 'کارد', 'کارد. (اِ) ابزار برنده.<br /></font></div>'),
(16, 'کارگر', 'کارگر. (ص مرکب) عمله و رنجبر.<br /></font></div>'),
(17, 'کارگاه', 'کارگاه. (اِ مرکب) جای کار.<br /></font></div>'),
(18, 'کاروان', 'کاروان. (اِ) گروه مسافران.<br /></font></div>'),
(19, 'بار', 'بار. (اِ) آنچه حمل کنند.<br /></font></div>'),
(20, 'باران', 'باران. (اِ) قطره های آب از آسمان.<br /></font></div>'),
(21, 'ابار', 'ابار. (ع اِ) انبارها.<br /></font></div>'),
(22, 'انبار', 'انبار. (اِ) جای نگهداری کالا و غله.<br /></font></div>'),
(23, 'علی', 'علی. (ع ص) بلند و والا.<br /></font></div>'),
(24, 'موسی', 'موسی. (اِخ) پیامبر بنی اسرائیل.<br /></font></div>'),
(25, 'خانه', 'خانه. (اِ) جای زندگی. بیت. غُرّه ٔ خانه ٔ من.<br /></font></div>'),
(26, 'آمدن', '<span class=\"hlight\" dir=\"rtl\" style=\"color:red\" onclick=\"evil()\">آمدن</span> <a id=\"fn1\" href=\"#\" onmouseover=\"javascript:alert(1)\">پانویس</a><script>alert(2)</script> <img src=x onerror=alert(3)> <font color=\"Red\" size=\"7\">قرمز</font> <font color=\"#00ff00\">سبز</font> <span class=\"evil\">x</span>');

ALTER TABLE `words`
  ADD PRIMARY KEY (`id`);
COMMIT;
