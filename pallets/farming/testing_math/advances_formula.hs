{-# LANGUAGE ScopedTypeVariables #-}

import System.IO
import System.Random
import Data.List
import System.IO.Unsafe

main = do
  candles <- readFile "bitcoin_daily.txt"
  let rows :: [[Double]] = map (map read . words) $ lines candles
  let gr = map (\a -> [a !! 3, a !! 4]) rows
  print "yes"

cut x v = if x > 0 then 0 else v

win_gen x a c = cut x $ ( exp x - exp (x*2) / a ) * sin ( x/b + pi*c )
 where
  b = pi / 2

win x a = win_gen x (1 + 0.5**(a*3)) (0.5 + a*0.5)

find_max :: (Double -> Double) -> IO Double
find_max win = f (-3) 0
 where
  smp = 4
  move = 2
  move1 = move+1
  f u v = do
    if abs(u-v) < 0.000000000001 then return v else do
        a <- mapM (\_ -> randomRIO (u,v)) [1..smp]
        let c = map (\x -> (x, win x)) (u:v:a)
        let d = fst $ foldr1 (\(a,b) (c,d) -> if b > d then (a,b) else (c,d)) c
        --print (u,v)
        let p = (u*move+d)/move1
        let q = (v*move+d)/move1
        if abs(u-p) > abs(v-q) then f p v else f u q

-- genTable 7.58 7.99 "/tmp/plot101_7.99_7.58"

getResults :: IO [Double]
getResults = do
  let list = reverse $ [0,0.001..1]
  let a = map (\a -> unsafePerformIO $ find_max (\x -> win x a)) list
  return a

genTable results p4 p5 p6 file = do
  let list = reverse $ [0,0.001..1]
  let a = results
  --let a = map (\a -> unsafePerformIO $ find_max (\x -> win x a)) list
  --let b = zipWith (\a b -> [b, win b a]) list a
  --let b = zipWith (\a b -> unwords $ map show $ [b, win b a]) list a
  let p2 = 4
  let p3 = 1
  let f b = let a = 1-b in ( exp (b / p2) - exp ((b/p2)*2) / (1 + 0.5**(a*3)) ) / p3 - sqrt ( ((p4-b)/p5)**2 + p6   )
  --let b = zipWith (\a x -> show $ log ( win x a - f (1-a) )) list a
  let b = zipWith (\a x -> show $ ( win x a - f (1-a) ) ) list a
  writeFile file $ unlines b




