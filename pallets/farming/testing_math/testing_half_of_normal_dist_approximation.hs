{-# LANGUAGE ScopedTypeVariables #-}

-- To test this run commands
-- ghc --make -package random testing_half_of_normal_dist_approximation.hs
-- ./testing_half_of_normal_dist_approximation > bitcoin_daily_test_plot.txt
-- run gnuplot program and enter commands
-- gnuplot> set style data lines
-- gnuplot> set logscale y
-- gnuplot> plot "bitcoin_daily_test_plot.txt" u 0:11 lw 8 lc "grey", "" u 0:12 lw 8 lc "grey", "" u 0:2, "" u 0:3, "" u 0:4, "" u 0:5, "" u 0:6, "" u 0:7, "" u 0:8, "" u 0:1, "" u 0:2, "" u 0:9, "" u 0:10

import System.IO
import System.Random
import Data.List

main = do
  candles <- readFile "bitcoin_daily.txt"
  let rows :: [[Double]] = map (map read . words) $ lines candles
  let gr = map (\a -> [a !! 3, a !! 4]) rows
  let win = 330
  let winH = win`div`2
  let powers = 2
  let agg_tab = reverse $ foldr (\pair list -> aggOp (16/realToFrac win) (head list) pair : list) [[replicate powers 0, replicate powers 1]] $ reverse gr
  let res = map (\a -> concat a) $ transpose ([drop win gr,drop winH $ apply f3 win gr, apply f2 win gr, drop (win) $ apply f1 win gr, apply f4 win gr] ++ map (drop (win)) (transpose $ map (fin.transpose) agg_tab) )
  mapM_ printRow $ reverse $ drop (win*2) $ reverse res

fin ([a,b]:[c,d]:_) = [[(a - c/2) * 2, (b - d/2) * 2]]

norm x = exp(-x*x)

f1 x = norm (x*6)

f2 x = norm ((1-x)*6)

f3 x = norm ((0.5-x)*6)

f4 _ = 1

aggOp :: Double -> [[Double]] -> [Double] -> [[Double]]
aggOp part [a,b] [c,d] = [prices,volumes]
 where
  parts = zipWith (\m _ -> part * m) [1..] a
  prices = map (\[a,b,part] -> (a * b  + c * d * part) / (b + d * part)) $ transpose [a,b,parts]
  volumes = map (\[b,part] -> (b + d * part) / (1 + part)) $ transpose [b,parts]

printRow :: [Double] -> IO ()
printRow list = do
  putStr ((unwords $ map show list) ++ "\n")

apply :: (Double -> Double) -> Int -> [[Double]] -> [[Double]]
apply fun win list 
  | length a < win = []
  | otherwise = [c/d,d/g] : apply fun win (tail list)
 where
  a = take win list
  fw :: Double
  fw = realToFrac win
  b = zipWith (\a [b,v] -> let w = fun (a/fw) * v in [b*w, w]) [0..(fw-1)] a
  [c,d] = foldr1 (\[a,b] [c,d] -> [a+c, b+d]) b
  g = foldr1 (+) $ map (\a -> fun (a/fw)) [0..(fw-1)]

