import Control.Monad
import Control.Monad.RWS (MonadState (put))
import Data.Array.Base
import Data.Array.IO
import GHC.IO.Handle
import GHC.IO.Handle.FD
import Text.Read

data Problem = Problem Double String Double

problems :: [Problem]
problems =
  [ Problem 1 "What was the gross world product in 2023 according to the World Bank, in USD?" 1.054e14,
    Problem 1 "What is the mass of the Earth, in kilograms?" 5.97e24,
    Problem 1 "What is the airspeed velocity of an unladen European swallow, in meters per second?" 11,
    Problem 1 "What is the mass of Earth's atmosphere, in kilograms?" 5.148e18,
    Problem 0.25 "What is the thermodynamic entropy of the Sun, in joules per kelvin? (weighted 25% of normal)" 1e35,
    Problem 1 "What was the population of the Earth in the year 1900 CE?" 1.6e9,
    Problem 1 "At its greatest extent, what portion of the Earth's land area was owned by the British Empire?" 0.2635
  ]

main :: IO ()
main = do
  hSetBuffering stdout NoBuffering

  printInstructions

  score <- sum <$> mapM askAndScore problems
  printTotalScore score

printTotalScore :: Double -> IO ()
printTotalScore score = do
  putStrLn ""
  putStrLn "Your Final Score"
  putStrLn "================"
  putStrLn $ "Your total score is: " ++ show score

askAndScore :: Problem -> IO Double
askAndScore problem = scoreAnswer problem <$> askQuestion problem

scoreAnswer :: Problem -> Double -> Double
scoreAnswer (Problem weight _ answer) guess = -weight * abs (log guess - log answer)

askQuestion :: Problem -> IO Double
askQuestion (Problem _ text answer) = do
  putStrLn ""
  putStrLn text
  putStrLn $ replicate (length text) '-'
  readAnswer

readAnswer :: IO Double
readAnswer = readAnswer' 0
  where
    readAnswer' :: Integer -> IO Double
    readAnswer' i = do
      putStr $ promptForIter i
      str <- readMaybe <$> getLine
      case str of
        Just x -> return x
        Nothing -> readAnswer' (i + 1)

    promptForIter 0 = "Your estimate: "
    promptForIter 1 = "Your estimate (invalid answer): "
    promptForIter _ = "Is it even possible to be this bad at entering numbers? Try again: "

printInstructions :: IO ()
printInstructions = do
  putStrLn "Fermi Estimation Challenge"
  putStrLn "=========================="
  putStrLn ""
  putStrLn "This program will ask you a series of questions with numerical answers."
  putStrLn "You will be scored according to the log-space error of your answers."
  putStrLn "That is, your score is -|ln(your_answer) - ln(correct_answer)|."
  putStrLn ""
  putStrLn "For this problem, you are not allowed to use the internet to look up any quantities."
  putStrLn "You are allowed to use Wolfram|Alpha, so long as you use it only as a calculator and not to look up physical quantities."
  putStrLn ""
  putStrLn "You can enter answers in scientific notation, e.g. 6.02e23."